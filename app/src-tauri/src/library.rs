//! Rutas y nombres al estilo Jellyfin. Mismas reglas que el script (`sanitize_filename`
//! y `get_episode_output_path`) para que app y terminal generen rutas idénticas:
//!
//! `<carpeta>/<Anime>/Season 01/<Anime> - S01E01.mp4`

use std::path::{Path, PathBuf};

/// Quita los caracteres inválidos en Windows (`< > : " / \ | ? *`) y los espacios o
/// puntos finales. Se aplica en todos los sistemas, igual que el script, para que la
/// biblioteca funcione en Windows y Linux. Conserva acentos y demás Unicode.
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !"<>:\"/\\|?*".contains(*c))
        .collect();
    cleaned
        .trim_end_matches(|c: char| c.is_whitespace() || c == '.')
        .to_string()
}

pub fn episode_filename(anime: &str, season: u32, episode: u32) -> String {
    format!(
        "{} - S{season:02}E{episode:02}.mp4",
        sanitize_filename(anime)
    )
}

pub fn episode_path(library: &Path, anime: &str, season: u32, episode: u32) -> PathBuf {
    let safe = sanitize_filename(anime);
    library
        .join(&safe)
        .join(format!("Season {season:02}"))
        .join(episode_filename(anime, season, episode))
}

/// Carpeta de descargas por defecto, como el script: `ANIME_DOWNLOAD_DIR` si existe,
/// si no `<carpeta de Vídeos>/Anime`.
pub fn default_library_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("ANIME_DOWNLOAD_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    let videos = dirs_videos().unwrap_or_else(|| home_dir().join("Videos"));
    videos.join("Anime")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Carpeta de Vídeos del usuario: XDG en Linux; en Windows, `%USERPROFILE%\Videos`.
fn dirs_videos() -> Option<PathBuf> {
    if cfg!(target_os = "linux") {
        let out = std::process::Command::new("xdg-user-dir")
            .arg("VIDEOS")
            .output()
            .ok()?;
        let p = String::from_utf8(out.stdout).ok()?.trim().to_string();
        let home = home_dir();
        (!p.is_empty() && Path::new(&p) != home).then(|| PathBuf::from(p))
    } else {
        Some(home_dir().join("Videos"))
    }
}
