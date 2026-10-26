//! Herramientas externas: yt-dlp (descargas) y ffmpeg/ffprobe (unir y validar vídeo).
//!
//! El instalador no las incluye. La app usa, por orden:
//! 1. su propia copia en `<datos de la app>/tools/` (descargada de las releases oficiales),
//! 2. la del sistema (`PATH`), por ejemplo la de pacman en Linux.
//!
//! Si falta alguna, la interfaz ofrece descargarla. En Windows se descargan las dos;
//! en Linux solo yt-dlp, porque ffmpeg lo instala el gestor de paquetes.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

pub const EVENT: &str = "tools-progress";

static TOOLS_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Llamar una vez al arrancar con la carpeta de datos de la app.
pub fn init(app_data: &Path) {
    let _ = TOOLS_DIR.set(app_data.join("tools"));
}

fn tools_dir() -> Option<&'static PathBuf> {
    TOOLS_DIR.get()
}

const EXE: &str = if cfg!(windows) { ".exe" } else { "" };

fn managed(name: &str) -> Option<PathBuf> {
    tools_dir()
        .map(|d| d.join(format!("{name}{EXE}")))
        .filter(|p| p.is_file())
}

fn in_path(name: &str) -> Option<PathBuf> {
    let file = format!("{name}{EXE}");
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(&file))
                .find(|p| p.is_file())
        })
        .unwrap_or(None)
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Copia propia de la app (se actualiza con `yt-dlp -U`).
    App,
    /// Instalada en el sistema.
    System,
    Missing,
}

fn locate(name: &str) -> (Option<PathBuf>, Source) {
    if let Some(p) = managed(name) {
        (Some(p), Source::App)
    } else if let Some(p) = in_path(name) {
        (Some(p), Source::System)
    } else {
        (None, Source::Missing)
    }
}

pub fn ytdlp() -> PathBuf {
    locate("yt-dlp")
        .0
        .unwrap_or_else(|| PathBuf::from(format!("yt-dlp{EXE}")))
}

pub fn ffprobe() -> Option<PathBuf> {
    locate("ffprobe").0
}

/// Carpeta de ffmpeg para `yt-dlp --ffmpeg-location`, si la app usa su propia copia.
pub fn managed_ffmpeg_dir() -> Option<PathBuf> {
    managed("ffmpeg").and_then(|p| p.parent().map(Path::to_path_buf))
}

/// Comando sin ventana de consola emergente en Windows.
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub source: Source,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolsStatus {
    pub ytdlp: ToolStatus,
    pub ffmpeg: ToolStatus,
    /// Si la app puede descargar ffmpeg en esta plataforma (solo Windows).
    pub can_install_ffmpeg: bool,
}

async fn version_of(path: &Path, arg: &str) -> Option<String> {
    let out = match command(path).arg(arg).output().await {
        Ok(o) => o,
        Err(e) => {
            log::warn!("no se pudo ejecutar {}: {e}", path.display());
            return None;
        }
    };
    if !out.status.success() {
        log::warn!(
            "{} {arg} terminó con {}: {}",
            path.display(),
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        );
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim().to_string();
    // "ffmpeg version N-118000-g... Copyright" → "N-118000-g..."
    Some(
        first
            .strip_prefix("ffmpeg version ")
            .map(|s| s.split(' ').next().unwrap_or(s).to_string())
            .unwrap_or(first),
    )
}

pub async fn status() -> ToolsStatus {
    let (yp, ys) = locate("yt-dlp");
    let (fp, fs) = locate("ffmpeg");
    let ytdlp = ToolStatus {
        source: ys,
        version: match &yp {
            Some(p) => version_of(p, "--version").await,
            None => None,
        },
    };
    let ffmpeg = ToolStatus {
        source: fs,
        version: match &fp {
            Some(p) => version_of(p, "-version").await,
            None => None,
        },
    };
    ToolsStatus {
        ytdlp,
        ffmpeg,
        can_install_ffmpeg: cfg!(windows),
    }
}

// ---------------------------------------------------------------- instalación

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub tool: &'static str,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub done: bool,
}

const YTDLP_URL: &str = if cfg!(windows) {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
} else if cfg!(target_arch = "aarch64") {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux_aarch64"
} else {
    "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
};

const FFMPEG_WIN_URL: &str = "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl-shared.zip";

async fn download<R: Runtime>(
    app: &AppHandle<R>,
    tool: &'static str,
    url: &str,
    dest: &Path,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent(concat!("the-wired/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("no se pudo descargar {tool}: {}", crate::error_chain(&e)))?;
    if !resp.status().is_success() {
        return Err(format!(
            "no se pudo descargar {tool}: HTTP {}",
            resp.status().as_u16()
        ));
    }
    let total = resp.content_length();
    let tmp = dest.with_extension("download");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| format!("no se pudo crear {}: {e}", tmp.display()))?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    let mut last_emit = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            format!(
                "descarga de {tool} interrumpida: {}",
                crate::error_chain(&e)
            )
        })?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("no se pudo escribir {}: {e}", tmp.display()))?;
        downloaded += chunk.len() as u64;
        if downloaded - last_emit > 512 * 1024 {
            last_emit = downloaded;
            let _ = app.emit(
                EVENT,
                Progress {
                    tool,
                    downloaded,
                    total,
                    done: false,
                },
            );
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);
    // En Windows el antivirus a veces bloquea el .exe recién bajado unos segundos
    rename_retrying(&tmp, dest).await?;
    log::info!(
        "{tool}: descargado ({downloaded} bytes) en {}",
        dest.display()
    );
    let _ = app.emit(
        EVENT,
        Progress {
            tool,
            downloaded,
            total,
            done: true,
        },
    );
    Ok(())
}

async fn rename_retrying(from: &Path, to: &Path) -> Result<(), String> {
    let mut last = None;
    for attempt in 0..5u64 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await;
        }
        match tokio::fs::rename(from, to).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                log::warn!(
                    "no se pudo mover {} a {}: {e}",
                    from.display(),
                    to.display()
                );
                last = Some(e);
            }
        }
    }
    Err(format!(
        "no se pudo guardar {} ({}). ¿Lo bloqueó el antivirus?",
        to.display(),
        last.map(|e| e.to_string()).unwrap_or_default()
    ))
}

/// Extrae del zip de FFmpeg-Builds el contenido de `bin/` (exe y dll) a `dir`.
fn extract_ffmpeg_bin(zip_path: &Path, dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let in_bin = name
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n == "bin");
        if !in_bin || entry.is_dir() {
            continue;
        }
        let Some(file_name) = name.file_name() else {
            continue;
        };
        let mut out = std::fs::File::create(dir.join(file_name)).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        found |= file_name == "ffmpeg.exe";
    }
    if found {
        Ok(())
    } else {
        Err("el zip de ffmpeg no tiene el formato esperado".into())
    }
}

/// Descarga lo que falte (o, con `force_ytdlp`, vuelve a bajar yt-dlp a la carpeta de la app).
pub async fn install_missing<R: Runtime>(app: &AppHandle<R>) -> Result<ToolsStatus, String> {
    let result = install_inner(app).await;
    match &result {
        Ok(s) => log::info!("herramientas instaladas: {s:?}"),
        Err(e) => log::error!("instalar herramientas: {e}"),
    }
    result
}

async fn install_inner<R: Runtime>(app: &AppHandle<R>) -> Result<ToolsStatus, String> {
    let dir = tools_dir()
        .ok_or("carpeta de herramientas no inicializada")?
        .clone();
    log::info!("instalando herramientas en {}", dir.display());
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("no se pudo crear {}: {e}", dir.display()))?;

    if locate("yt-dlp").1 == Source::Missing {
        let dest = dir.join(format!("yt-dlp{EXE}"));
        download(app, "yt-dlp", YTDLP_URL, &dest).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| e.to_string())?;
        }
    }

    if cfg!(windows) && locate("ffmpeg").1 == Source::Missing {
        let zip_path = dir.join("ffmpeg.zip");
        download(app, "ffmpeg", FFMPEG_WIN_URL, &zip_path).await?;
        let (z, d) = (zip_path.clone(), dir.clone());
        tokio::task::spawn_blocking(move || extract_ffmpeg_bin(&z, &d))
            .await
            .map_err(|e| e.to_string())??;
        let _ = tokio::fs::remove_file(&zip_path).await;
    }

    Ok(status().await)
}

/// Ejecuta `yt-dlp -U`. Solo funciona con la copia de la app; la del sistema se
/// actualiza con el gestor de paquetes.
pub async fn update_ytdlp() -> Result<String, String> {
    let (path, source) = locate("yt-dlp");
    match (path, source) {
        (Some(p), Source::App) => {
            let out = command(&p).arg("-U").output().await.map_err(|e| e.to_string())?;
            let text = format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            log::info!("yt-dlp -U ({}):\n{}", out.status, text.trim());
            let last = text.lines().map(str::trim).rfind(|l| !l.is_empty()).unwrap_or("").to_string();
            if out.status.success() { Ok(last) } else { Err(last) }
        }
        (Some(_), Source::System) => Err(
            "Este yt-dlp lo instaló el sistema: actualízalo con tu gestor de paquetes (en Arch/CachyOS: paru).".into(),
        ),
        _ => Err("yt-dlp no está instalado.".into()),
    }
}
