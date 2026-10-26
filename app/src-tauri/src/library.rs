//! Rutas y nombres al estilo Jellyfin. Mismas reglas que el script (`sanitize_filename`
//! y `get_episode_output_path`) para que app y terminal generen rutas idénticas:
//!
//! `<carpeta>/<Anime>/Season 01/<Anime> - S01E01.mp4`
//!
//! Sin el formato Jellyfin (ajuste `jellyfin_layout`), todo va en la carpeta del anime:
//!
//! `<carpeta>/<Anime>/<Anime> - 01.mp4` (y `<carpeta>/<Anime>/<Anime>.mp4` si es película)

use std::path::{Path, PathBuf};

use regex::Regex;

/// Cómo se organizan las descargas dentro de la carpeta de la biblioteca.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// `<Anime>/Season 01/<Anime> - S01E01.mp4`, lo que espera Jellyfin (y el script).
    Jellyfin,
    /// `<Anime>/<Anime> - 01.mp4`, sin temporadas.
    Simple,
}

impl Layout {
    pub fn from_jellyfin(jellyfin: bool) -> Self {
        if jellyfin {
            Self::Jellyfin
        } else {
            Self::Simple
        }
    }
}

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

/// Ruta de un episodio según el formato elegido.
pub fn output_path(
    library: &Path,
    layout: Layout,
    anime: &str,
    is_movie: bool,
    season: u32,
    episode: u32,
) -> PathBuf {
    match layout {
        Layout::Jellyfin => episode_path(library, anime, season, episode),
        Layout::Simple => {
            let safe = sanitize_filename(anime);
            let file = if is_movie {
                format!("{safe}.mp4")
            } else {
                format!("{safe} - {episode:02}.mp4")
            };
            library.join(&safe).join(file)
        }
    }
}

/// Temporadas (`Season NN`) con algún `.mp4` dentro de la carpeta del anime.
pub fn seasons_on_disk(library: &Path, anime: &str) -> Vec<u32> {
    let re = Regex::new(r"^Season (\d+)$").unwrap();
    let Ok(entries) = std::fs::read_dir(library.join(sanitize_filename(anime))) else {
        return Vec::new();
    };
    let mut seasons: Vec<u32> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let n = re.captures(&name)?[1].parse().ok()?;
            let has_video = std::fs::read_dir(e.path()).ok()?.flatten().any(|f| {
                let f = f.file_name().to_string_lossy().to_string();
                f.ends_with(".mp4") && !f.contains(".part")
            });
            has_video.then_some(n)
        })
        .collect();
    seasons.sort_unstable();
    seasons
}

/// jkanime publica cada temporada como un anime aparte ("Overlord II",
/// "Shingeki no Kyojin Season 2", "Kimetsu no Yaiba 2nd Season"...). Separa el nombre
/// de la serie y el número de temporada para guardarlo donde Jellyfin lo espera.
/// Si el título no indica temporada, devuelve el título tal cual y la temporada 1.
pub fn split_season(title: &str) -> (String, u32) {
    let patterns = [
        // "Season 2", "Temporada 2", "2nd Season", "2da Temporada"
        r"(?i)^(.+?)[\s:\-]+(?:season|temporada)\s+(\d{1,2})\b.*$",
        r"(?i)^(.+?)[\s:\-]+(\d{1,2})(?:st|nd|rd|th|da|ra|ta|ma|na|va)?\s+(?:season|temporada)\b.*$",
    ];
    for p in patterns {
        if let Some(c) = Regex::new(p).unwrap().captures(title)
            && let Ok(n) = c[2].parse::<u32>()
            && n >= 1
        {
            return (c[1].trim().to_string(), n);
        }
    }
    // Números romanos al final: "Overlord II"
    let roman = [("II", 2), ("III", 3), ("IV", 4), ("V", 5), ("VI", 6)];
    if let Some((base, last)) = title.trim().rsplit_once(' ')
        && let Some((_, n)) = roman.iter().find(|(r, _)| *r == last)
    {
        return (base.trim().to_string(), *n);
    }
    (title.trim().to_string(), 1)
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
