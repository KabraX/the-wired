//! YouTube sin claves de API: se usa el `yt-dlp` de la app para buscar (`ytsearch`),
//! leer la ficha o la playlist (`--dump-single-json`) y descargar el vídeo o el audio.
//!
//! Igual que en `jkanime`, las funciones `parse_*` son puras (reciben el JSON y
//! devuelven datos) para poder probarlas sin red con respuestas guardadas.

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::jkanime::{JkError, Result, Site, search::SearchResult};
use crate::tools;

/// Resultados por página en la búsqueda por texto.
pub const PER_PAGE: u32 = 12;

/// Cómo se descarga un vídeo de YouTube.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum YtFormat {
    /// MP4 con la mejor pista de vídeo (hasta `max_height`, si se indica) y audio.
    Video { max_height: Option<u32> },
    /// Solo audio, extraído a MP3 o M4A.
    Audio { codec: AudioCodec },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Mp3,
    M4a,
}

impl YtFormat {
    /// Extensión del archivo final (la usa la cola para saber qué esperar en disco).
    pub fn extension(&self) -> &'static str {
        match self {
            YtFormat::Video { .. } => "mp4",
            YtFormat::Audio {
                codec: AudioCodec::Mp3,
            } => "mp3",
            YtFormat::Audio {
                codec: AudioCodec::M4a,
            } => "m4a",
        }
    }
}

/// Un vídeo concreto que se puede descargar (suelto o dentro de una playlist).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YtItem {
    pub url: String,
    pub title: String,
    /// Posición en la playlist (para el nombre `01 - Título.mp4`).
    #[serde(default)]
    pub index: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YtEntry {
    pub index: u32,
    pub id: String,
    pub title: String,
    pub url: String,
    pub duration: Option<u64>,
}

/// Ficha de un vídeo o de una playlist (lo que muestra la página de YouTube).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct YtInfo {
    pub url: String,
    pub title: String,
    pub image: Option<String>,
    pub channel: Option<String>,
    pub duration: Option<u64>,
    pub is_playlist: bool,
    /// Los vídeos de la playlist o, si es un vídeo suelto, ese único vídeo.
    pub entries: Vec<YtEntry>,
}

/// ¿La entrada es una URL de YouTube (vídeo, playlist o enlace corto)?
pub fn is_youtube_url(input: &str) -> bool {
    Site::from_url(input) == Some(Site::Youtube)
}

/// URL de un vídeo reproducible: `/watch?v=`, `/shorts/`, `/embed/` o `youtu.be/…`.
pub fn is_video_url(url: &str) -> bool {
    let Ok(parsed) = url::Url::parse(url.trim()) else {
        return false;
    };
    if !is_youtube_url(url) {
        return false;
    }
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if host.ends_with("youtu.be") {
        return parsed.path().len() > 1;
    }
    parsed.path() == "/watch"
        || parsed.path().starts_with("/shorts/")
        || parsed.path().starts_with("/live/")
        || parsed.path().starts_with("/embed/")
}

/// URLs que la app sabe abrir en su ficha: un vídeo o una playlist (ni canales ni
/// búsquedas, que `yt-dlp` expandiría sin límite).
pub fn is_supported_url(url: &str) -> bool {
    if is_video_url(url) {
        return true;
    }
    let Ok(parsed) = url::Url::parse(url.trim()) else {
        return false;
    };
    is_youtube_url(url) && parsed.path() == "/playlist"
}

/// "1:02:03" o "4:05", para la tarjeta de resultados.
pub fn format_duration(seconds: u64) -> String {
    let (h, m, s) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn as_u64(v: Option<&Value>) -> Option<u64> {
    match v? {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn text(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| v.get(*k).and_then(Value::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn image_of(v: &Value) -> Option<String> {
    if let Some(t) = text(v, &["thumbnail"]) {
        return Some(t);
    }
    v.get("thumbnails")
        .and_then(Value::as_array)
        .and_then(|list| list.last())
        .and_then(|t| t.get("url"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Enlace del vídeo: `webpage_url`, una URL absoluta o, si no, el id.
fn entry_url(v: &Value) -> Option<String> {
    if let Some(u) = text(v, &["webpage_url"])
        && u.starts_with("http")
    {
        return Some(u);
    }
    if let Some(u) = text(v, &["url"])
        && u.starts_with("http")
    {
        return Some(u);
    }
    v.get("id")
        .and_then(Value::as_str)
        .map(|id| format!("https://www.youtube.com/watch?v={id}"))
}

/// Los vídeos borrados o privados llegan como "[Private video]" y no se pueden bajar.
fn unavailable(title: &str) -> bool {
    title.starts_with('[') && title.ends_with(']')
}

fn entry_of(v: &Value, index: u32) -> Option<YtEntry> {
    let url = entry_url(v)?;
    if !is_video_url(&url) {
        return None;
    }
    let title = text(v, &["title", "fulltitle"])?;
    if unavailable(&title) {
        return None;
    }
    Some(YtEntry {
        index: v
            .get("playlist_index")
            .and_then(|n| as_u64(Some(n)))
            .map(|n| n as u32)
            .unwrap_or(index),
        id: text(v, &["id"]).unwrap_or_default(),
        title,
        url,
        duration: as_u64(v.get("duration")),
    })
}

/// Interpreta la respuesta de `yt-dlp --flat-playlist --dump-single-json ytsearch…`.
pub fn parse_search_json(json: &str) -> Result<Vec<SearchResult>> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| JkError::SiteChanged(format!("respuesta de YouTube inesperada: {e}")))?;
    let single = value.get("entries").and_then(Value::as_array).is_none();
    let entries: Vec<&Value> = match value.get("entries").and_then(Value::as_array) {
        Some(list) => list.iter().collect(),
        None => vec![&value],
    };
    Ok(entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let entry = entry_of(e, i as u32 + 1)?;
            let image = image_of(e).or_else(|| {
                (!entry.id.is_empty())
                    .then(|| format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", entry.id))
            });
            Some(SearchResult {
                source: Site::Youtube,
                title: entry.title,
                url: entry.url,
                image,
                kind: entry.duration.map(format_duration).or_else(|| {
                    single.then(|| match text(&value, &["_type"]).as_deref() {
                        Some("playlist") => "Playlist".to_string(),
                        _ => "Vídeo".to_string(),
                    })
                }),
                status: text(e, &["channel", "uploader"]),
            })
        })
        .collect())
}

/// Interpreta la ficha de `yt-dlp --dump-single-json <url>`.
pub fn parse_info_json(json: &str, fallback_url: &str) -> Result<YtInfo> {
    let value: Value = serde_json::from_str(json)
        .map_err(|e| JkError::SiteChanged(format!("respuesta de YouTube inesperada: {e}")))?;
    let is_playlist = text(&value, &["_type"]).as_deref() == Some("playlist");
    let url = text(&value, &["webpage_url", "original_url", "url"])
        .filter(|u| u.starts_with("http"))
        .unwrap_or_else(|| fallback_url.trim().to_string());
    let title = text(&value, &["title", "fulltitle"])
        .ok_or_else(|| JkError::SiteChanged("la respuesta de YouTube no tiene título".into()))?;
    let entries: Vec<YtEntry> = if is_playlist {
        value
            .get("entries")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .enumerate()
                    .filter_map(|(i, e)| entry_of(e, i as u32 + 1))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        entry_of(&value, 1).into_iter().collect()
    };
    Ok(YtInfo {
        url,
        title,
        image: image_of(&value),
        channel: text(&value, &["channel", "uploader"]),
        duration: as_u64(value.get("duration")),
        is_playlist,
        entries,
    })
}

/// Argumentos de `yt-dlp` para bajar un vídeo de YouTube al archivo `template`.
pub fn download_args(
    url: &str,
    template: &Path,
    format: &YtFormat,
    progress_template: &str,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "--newline",
        "--no-colors",
        "--no-playlist",
        "--concurrent-fragments",
        "4",
        "--progress-template",
        progress_template,
        "-o",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(template.as_os_str().to_owned());
    match format {
        YtFormat::Video { max_height } => {
            let selector = match max_height {
                Some(h) => format!(
                    "bv*[ext=mp4][height<={h}]+ba[ext=m4a]/b[ext=mp4][height<={h}]/\
                     bv*[height<={h}]+ba/b[height<={h}]/b"
                ),
                None => "bv*[ext=mp4]+ba[ext=m4a]/b[ext=mp4]/bv*+ba/b".to_string(),
            };
            args.extend(
                ["--merge-output-format", "mp4", "--remux-video", "mp4", "-f"]
                    .into_iter()
                    .map(OsString::from),
            );
            args.push(selector.into());
        }
        YtFormat::Audio { codec } => {
            let audio = match codec {
                AudioCodec::Mp3 => "mp3",
                AudioCodec::M4a => "m4a",
            };
            args.extend(
                ["-x", "--audio-format", audio, "--audio-quality", "0"]
                    .into_iter()
                    .map(OsString::from),
            );
        }
    }
    args.push(url.trim().into());
    args
}

/// Fallos que no se arreglan reintentando (vídeo privado, borrado, restringido…).
pub fn is_permanent_error(message: &str) -> bool {
    const PERMANENT: [&str; 8] = [
        "private video",
        "video unavailable",
        "this video is not available",
        "sign in to confirm",
        "unsupported url",
        "not available in your country",
        "members-only",
        "has been removed",
    ];
    let message = message.to_ascii_lowercase();
    PERMANENT.iter().any(|p| message.contains(p))
}

/// Ejecuta `yt-dlp --dump-single-json` sobre una URL o búsqueda y devuelve el JSON.
async fn dump(target: &str) -> Result<String> {
    let mut cmd = tools::command(tools::ytdlp());
    cmd.args([
        "--no-warnings",
        "--no-colors",
        "--flat-playlist",
        "--skip-download",
        "--dump-single-json",
        target,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let output = tokio::time::timeout(Duration::from_secs(120), cmd.output())
        .await
        .map_err(|_| JkError::Unavailable("yt-dlp tardó demasiado en responder".into()))?
        .map_err(|e| JkError::Unavailable(format!("no se pudo ejecutar yt-dlp: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let last = stderr
            .lines()
            .rev()
            .find(|l| l.contains("ERROR"))
            .map(str::trim)
            .unwrap_or("");
        let message = if last.is_empty() {
            "yt-dlp no pudo leer la página de YouTube".to_string()
        } else {
            last.to_string()
        };
        return Err(if is_permanent_error(&message) {
            JkError::NotFound(message)
        } else {
            JkError::Unavailable(message)
        });
    }
    String::from_utf8(output.stdout).map_err(|e| JkError::Unavailable(e.to_string()))
}

/// Ficha de un vídeo o de una playlist de YouTube.
pub async fn fetch_info(url: &str) -> Result<YtInfo> {
    let url = url.trim();
    if !is_supported_url(url) {
        return Err(JkError::NotFound(format!(
            "no es una URL de vídeo o playlist de YouTube: {url}"
        )));
    }
    let json = dump(url).await?;
    parse_info_json(&json, url)
}

/// Busca en YouTube. Si la consulta es una URL, devuelve ese vídeo o playlist.
pub async fn search(query: &str, page: u32) -> Result<Vec<SearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    if is_youtube_url(query) {
        let info = fetch_info(query).await?;
        return Ok(vec![SearchResult {
            source: Site::Youtube,
            title: info.title,
            url: info.url,
            image: info.image,
            kind: Some(
                if info.is_playlist {
                    "Playlist"
                } else {
                    "Vídeo"
                }
                .to_string(),
            ),
            status: info.channel,
        }]);
    }
    let page = page.clamp(1, 10);
    let json = dump(&format!("ytsearch{}:{query}", page * PER_PAGE)).await?;
    let skip = ((page - 1) * PER_PAGE) as usize;
    Ok(parse_search_json(&json)?.into_iter().skip(skip).collect())
}
