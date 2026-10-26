pub mod download;
pub mod jkanime;
pub mod library;
pub mod settings;
pub mod tools;

use std::path::PathBuf;
use std::sync::Mutex;

use download::{DownloadManager, EpisodeRequest, Job};
use jkanime::{JkClient, JkError, anime::AnimeInfo, episode::VideoSource, search::SearchResult};
use serde::{Deserialize, Serialize};
use settings::Settings;
use tauri::{AppHandle, Manager, State};

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
    query: String,
    page: Option<u32>,
) -> Result<Vec<SearchResult>, JkError> {
    jkanime::search::search(&client, &query, page.unwrap_or(1)).await
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
    is_movie: bool,
    season: u32,
    episodes: Vec<u32>,
}

#[tauri::command]
fn enqueue_downloads(
    app: AppHandle,
    client: State<'_, JkClient>,
    downloads: State<'_, DownloadManager>,
    settings: State<'_, SettingsState>,
    args: DownloadArgs,
) -> Vec<u64> {
    let library_dir = settings.settings.lock().unwrap().library_dir.clone();
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
    downloads.enqueue(&app, client.inner().clone(), &library_dir, requests)
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
fn clear_finished_downloads(downloads: State<'_, DownloadManager>) {
    downloads.clear_finished();
}

#[tauri::command]
async fn episodes_on_disk(
    settings: State<'_, SettingsState>,
    anime: String,
    season: u32,
    episodes: Vec<u32>,
) -> Result<Vec<u32>, String> {
    let library_dir = settings.settings.lock().unwrap().library_dir.clone();
    Ok(download::episodes_on_disk(&library_dir, &anime, season.max(1), &episodes).await)
}

/// Carpeta del anime en la biblioteca (para el botón "Abrir carpeta").
#[tauri::command]
fn anime_folder(settings: State<'_, SettingsState>, anime: String) -> String {
    let library_dir = settings.settings.lock().unwrap().library_dir.clone();
    library_dir
        .join(library::sanitize_filename(&anime))
        .display()
        .to_string()
}

#[tauri::command]
async fn update_ytdlp() -> Result<String, String> {
    tools::update_ytdlp().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(JkClient::new())
        .setup(|app| {
            tools::init(&app.path().app_data_dir()?);
            let path = app.path().app_config_dir()?.join("settings.json");
            let settings = Settings::load(&path).sanitized();
            app.manage(DownloadManager::new(settings.concurrent_downloads));
            app.manage(SettingsState {
                settings: Mutex::new(settings),
                path,
            });
            Ok(())
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
            episodes_on_disk,
            anime_folder,
            update_ytdlp,
            tools_status,
            install_tools,
        ])
        .run(tauri::generate_context!())
        .expect("error al arrancar the-wired");
}
