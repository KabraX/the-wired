pub mod catalog;
pub mod download;
pub mod jkanime;
pub mod library;
pub mod settings;
pub mod tools;
mod tray;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use download::{DownloadManager, EpisodeRequest, Job};
use jkanime::{
    JkClient, JkError, Site, anime::AnimeInfo, episode::VideoSource, search::SearchResult,
};
use serde::{Deserialize, Serialize};
use settings::Settings;
use tauri::{AppHandle, Manager, State};

/// Nombre del archivo de registro (sin `.log`) en la carpeta de logs de la app.
const LOG_FILE: &str = "the-wired";

/// Un error con todas sus causas: reqwest, por ejemplo, solo dice "error sending request"
/// y el motivo real (DNS, TLS, proxy…) está en `source()`.
pub fn error_chain(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut cur = e.source();
    while let Some(s) = cur {
        text.push_str(": ");
        text.push_str(&s.to_string());
        cur = s.source();
    }
    text
}

/// Si hay icono en la bandeja (si no, cerrar la ventana cierra la app).
struct HasTray(bool);

/// Ajustes en memoria más la ruta del archivo donde se guardan.
struct SettingsState {
    settings: Mutex<Settings>,
    path: PathBuf,
}

/// Datos básicos de la app, para comprobar que la interfaz y Rust se comunican.
#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    os: &'static str,
    arch: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

#[tauri::command]
async fn tools_status() -> tools::ToolsStatus {
    tools::status().await
}

#[tauri::command]
async fn install_tools(app: AppHandle) -> Result<tools::ToolsStatus, String> {
    tools::install_missing(&app).await
}

// ------------------------------------------------------------------ jkanime

#[tauri::command]
async fn search(
    client: State<'_, JkClient>,
    settings: State<'_, SettingsState>,
    source: Site,
    query: String,
    page: Option<u32>,
) -> Result<Vec<SearchResult>, JkError> {
    if !settings.settings.lock().unwrap().source(source).enabled {
        return Ok(Vec::new());
    }
    jkanime::search::search(&client, source, &query, page.unwrap_or(1)).await
}

#[tauri::command]
async fn anime(client: State<'_, JkClient>, url: String) -> Result<AnimeInfo, JkError> {
    jkanime::anime::fetch_anime(&client, &url).await
}

#[tauri::command]
async fn video_source(
    client: State<'_, JkClient>,
    anime_url: String,
    number: u32,
    is_movie: bool,
) -> Result<VideoSource, JkError> {
    jkanime::episode::fetch_video_source(&client, &anime_url, number, is_movie).await
}

// ------------------------------------------------------------------ ajustes

#[tauri::command]
fn get_settings(state: State<'_, SettingsState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(state: State<'_, SettingsState>, settings: Settings) -> Result<Settings, String> {
    let settings = settings.sanitized();
    settings.save(&state.path).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = settings.clone();
    Ok(settings)
}

// ------------------------------------------------------------------ descargas

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadArgs {
    anime: String,
    anime_url: String,
    /// Título en el sitio y portada, para la biblioteca.
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    image: Option<String>,
    is_movie: bool,
    season: u32,
    episodes: Vec<u32>,
}

#[tauri::command]
fn enqueue_downloads(
    app: AppHandle,
    downloads: State<'_, DownloadManager>,
    settings: State<'_, SettingsState>,
    index: State<'_, Arc<catalog::Index>>,
    args: DownloadArgs,
) -> Vec<u64> {
    // Cada sitio guarda en su propia biblioteca
    let site = Site::from_url(&args.anime_url).unwrap_or_default();
    index.record(catalog::IndexEntry {
        source: site,
        folder: args.anime.clone(),
        title: args.title.clone().unwrap_or_else(|| args.anime.clone()),
        url: args.anime_url.clone(),
        image: args.image.clone(),
    });
    let (library_dir, layout) = settings.settings.lock().unwrap().library(site);
    let season = args.season.max(1);
    let requests = args
        .episodes
        .into_iter()
        .map(|episode| EpisodeRequest {
            anime: args.anime.clone(),
            anime_url: args.anime_url.clone(),
            is_movie: args.is_movie,
            season,
            episode,
        })
        .collect();
    downloads.enqueue(&app, &library_dir, layout, requests)
}

#[tauri::command]
fn list_downloads(downloads: State<'_, DownloadManager>) -> Vec<Job> {
    downloads.jobs()
}

#[tauri::command]
fn cancel_download(app: AppHandle, downloads: State<'_, DownloadManager>, id: u64) {
    downloads.cancel(&app, id);
}

#[tauri::command]
fn pause_download(app: AppHandle, downloads: State<'_, DownloadManager>, id: u64) {
    downloads.pause(&app, id);
}

/// Reanuda una pausada o reintenta una fallida/cancelada.
#[tauri::command]
fn resume_download(app: AppHandle, downloads: State<'_, DownloadManager>, id: u64) {
    downloads.resume(&app, id);
}

#[tauri::command]
fn pause_all_downloads(app: AppHandle, downloads: State<'_, DownloadManager>) {
    downloads.pause_all(&app);
}

#[tauri::command]
fn resume_all_downloads(app: AppHandle, downloads: State<'_, DownloadManager>) {
    downloads.resume_all(&app);
}

#[tauri::command]
fn clear_finished_downloads(downloads: State<'_, DownloadManager>) {
    downloads.clear_finished();
}

#[tauri::command]
async fn episodes_on_disk(
    settings: State<'_, SettingsState>,
    source: Site,
    anime: String,
    is_movie: bool,
    season: u32,
    episodes: Vec<u32>,
) -> Result<Vec<u32>, String> {
    let (library_dir, layout) = settings.settings.lock().unwrap().library(source);
    Ok(download::episodes_on_disk(
        &library_dir,
        layout,
        &anime,
        is_movie,
        season.max(1),
        &episodes,
    )
    .await)
}

/// Temporadas de un anime que ya tienen episodios en la biblioteca (formato Jellyfin).
#[tauri::command]
fn seasons_on_disk(settings: State<'_, SettingsState>, source: Site, anime: String) -> Vec<u32> {
    let (library_dir, _) = settings.settings.lock().unwrap().library(source);
    library::seasons_on_disk(&library_dir, &anime)
}

/// Animes de las bibliotecas de las fuentes activas (lo más reciente primero).
#[tauri::command]
async fn scan_library(
    settings: State<'_, SettingsState>,
    index: State<'_, Arc<catalog::Index>>,
) -> Result<Vec<catalog::LibraryAnime>, String> {
    let dirs: Vec<(Site, PathBuf)> = {
        let s = settings.settings.lock().unwrap();
        Site::ALL
            .into_iter()
            .filter(|site| s.source(*site).enabled)
            .map(|site| (site, s.library(site).0))
            .collect()
    };
    let index = index.inner().clone();
    // Leer carpetas (quizá en un disco USB) fuera del hilo de la interfaz
    tauri::async_runtime::spawn_blocking(move || {
        dirs.iter()
            .flat_map(|(site, dir)| catalog::scan(*site, dir, &index))
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Completa el enlace y la portada de un anime de la biblioteca: lee su ficha si ya
/// se conoce el enlace o, si no (bajado con el script), lo busca por nombre y lo
/// acepta si el título coincide exactamente o es el único resultado. Lo encontrado queda en el índice.
#[tauri::command]
async fn resolve_library_anime(
    client: State<'_, JkClient>,
    index: State<'_, Arc<catalog::Index>>,
    source: Site,
    name: String,
) -> Result<Option<catalog::IndexEntry>, JkError> {
    let known = index.get(source, &name);
    let (url, title) = match &known {
        Some(e) => (e.url.clone(), e.title.clone()),
        None => {
            let query = catalog::folder_query(&name);
            let want = catalog::normalize_title(&query);
            let mut found = None;
            // La carpeta no lleva los signos que Windows no admite ("Anehame: Ore no…" se
            // guarda como "Anehame Ore no…") y la búsqueda de la web no lo encuentra así:
            // si la consulta completa falla, se prueba con las primeras palabras
            for (i, q) in catalog::search_queries(&query).iter().enumerate() {
                let mut results = jkanime::search::search(&client, source, q, 1).await?;
                let exact = results
                    .iter()
                    .position(|r| catalog::normalize_title(&r.title) == want);
                // Con la consulta completa vale también un único resultado
                // ("Junji Ito Collection" → "Itou Junji: Collection")
                let pick = exact.or((i == 0 && results.len() == 1).then_some(0));
                if let Some(i) = pick {
                    found = Some(results.swap_remove(i));
                    break;
                }
            }
            let Some(hit) = found else {
                log::info!("biblioteca: no se encontró «{name}» en {source:?}");
                return Ok(None);
            };
            (hit.url, hit.title)
        }
    };
    let html = client.get_text(&url, None).await?;
    let page = jkanime::anime::parse_anime_page(&html);
    let entry = catalog::IndexEntry {
        source,
        folder: name,
        title: page.title.unwrap_or(title),
        url,
        image: page.image,
    };
    index.record(entry.clone());
    Ok(Some(entry))
}

/// Nombre de la serie y temporada que se deducen del título de jkanime.
#[tauri::command]
fn split_season(title: String) -> (String, u32) {
    library::split_season(&title)
}

/// Carpeta del anime en la biblioteca (para el botón "Abrir carpeta").
#[tauri::command]
fn anime_folder(settings: State<'_, SettingsState>, source: Site, anime: String) -> String {
    let (library_dir, _) = settings.settings.lock().unwrap().library(source);
    library_dir
        .join(library::sanitize_filename(&anime))
        .display()
        .to_string()
}

/// Registro para adjuntar a un aviso de error: datos del equipo, herramientas y las
/// últimas líneas del archivo de log.
#[tauri::command]
async fn read_log(app: AppHandle) -> Result<String, String> {
    let path = app
        .path()
        .app_log_dir()
        .map_err(|e| e.to_string())?
        .join(format!("{LOG_FILE}.log"));
    let tools = tools::status().await;
    let text = tokio::fs::read_to_string(&path).await.unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let tail = lines[lines.len().saturating_sub(400)..].join("\n");
    Ok(format!(
        "the-wired {} · {} {}\nyt-dlp: {:?} {}\nffmpeg: {:?} {}\nRegistro: {}\n\n{tail}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        tools.ytdlp.source,
        tools.ytdlp.version.as_deref().unwrap_or("-"),
        tools.ffmpeg.source,
        tools.ffmpeg.version.as_deref().unwrap_or("-"),
        path.display(),
    ))
}

/// Ruta del archivo de registro (para "Mostrar en la carpeta").
#[tauri::command]
fn log_file(app: AppHandle) -> Result<String, String> {
    let dir = app.path().app_log_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{LOG_FILE}.log")).display().to_string())
}

#[tauri::command]
async fn update_ytdlp() -> Result<String, String> {
    tools::update_ytdlp().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir {
                        file_name: Some(LOG_FILE.into()),
                    }),
                ])
                // Lo nuestro con detalle; de las librerías, solo avisos
                .level(log::LevelFilter::Warn)
                .level_for("the_wired_lib", log::LevelFilter::Debug)
                .level_for("webview", log::LevelFilter::Info)
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepOne)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(JkClient::new())
        .setup(|app| {
            log::info!(
                "the-wired {} arranca en {} {}",
                env!("CARGO_PKG_VERSION"),
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            tools::init(&app.path().app_data_dir()?);
            tauri::async_runtime::spawn(async {
                let s = tools::status().await;
                log::info!(
                    "herramientas: yt-dlp {:?} {:?} · ffmpeg {:?} {:?}",
                    s.ytdlp.source,
                    s.ytdlp.version,
                    s.ffmpeg.source,
                    s.ffmpeg.version
                );
            });
            let path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&path).sanitized();
            let queue_file = app.path().app_data_dir()?.join("downloads.json");
            let client = app.state::<JkClient>().inner().clone();
            let mut manager =
                DownloadManager::new(settings.concurrent_downloads, client, Some(queue_file));
            // Bandeja opcional: sin ella la app funciona igual (cerrar = salir)
            let tray = tray::available()
                .then(|| tray::build(app.handle()).ok())
                .flatten();
            app.manage(HasTray(tray.is_some()));
            if tray.is_some() {
                // El icono solo se toca desde el hilo principal: TrayIcon usa Rc por dentro
                // y clonarlo desde los hilos de las descargas corrompía su contador (abort).
                let handle = app.handle().clone();
                manager = manager.with_hook(move |jobs| {
                    let jobs = jobs.to_vec();
                    let h = handle.clone();
                    let _ = handle.run_on_main_thread(move || {
                        if let Some(tray) = h.tray_by_id(tray::ID) {
                            tray::update(&tray, &jobs);
                        }
                    });
                });
            }
            // Índice de la biblioteca (enlace y portada de cada anime)
            let index = Arc::new(catalog::Index::load(Some(
                app.path().app_data_dir()?.join("library.json"),
            )));
            app.manage(index.clone());
            app.manage(manager.clone());
            app.manage(SettingsState {
                settings: Mutex::new(settings),
                path,
            });
            // Retoma la cola guardada (lo que quedó a medias al cerrar)
            manager.restore(app.handle());
            // Lo descargado antes de existir el índice: sale de la cola guardada
            for job in manager.jobs() {
                let source = Site::from_url(&job.anime_url).unwrap_or_default();
                if index.get(source, &job.anime).is_none() {
                    index.record(catalog::IndexEntry {
                        source,
                        folder: job.anime.clone(),
                        title: job.anime.clone(),
                        url: job.anime_url.clone(),
                        image: None,
                    });
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Con descargas en marcha, cerrar la ventana la esconde en la bandeja
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if app.state::<HasTray>().0 && app.state::<DownloadManager>().has_active() {
                    api.prevent_close();
                    let _ = window.hide();
                    tray::notify_background(app);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            search,
            anime,
            video_source,
            get_settings,
            set_settings,
            enqueue_downloads,
            list_downloads,
            cancel_download,
            clear_finished_downloads,
            pause_download,
            resume_download,
            pause_all_downloads,
            resume_all_downloads,
            episodes_on_disk,
            seasons_on_disk,
            scan_library,
            resolve_library_anime,
            split_season,
            anime_folder,
            update_ytdlp,
            read_log,
            log_file,
            tools_status,
            install_tools,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar the-wired");
}
