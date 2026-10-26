//! Fuente de vídeo de un episodio. Misma cadena y orden que `scrape_episode` del script:
//! 1. jkplayer (método actual) → 2. jk.php (legacy) → 3. Mediafire desde `var servers`.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::{BASE_URL, JkClient, JkError, Result};

/// De dónde salió la URL, para mostrarlo y para depurar cuando la web cambie.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Jkplayer,
    Jkphp,
    Mediafire,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VideoSource {
    /// `.m3u8` (HLS) o enlace directo de descarga.
    pub url: String,
    pub kind: SourceKind,
    /// Página desde la que se pidió el vídeo; algunos servidores exigen este `Referer`.
    pub referer: String,
}

/// `https://jkanime.net/<anime>/<n>/` o `/pelicula/` para películas.
pub fn episode_url(anime_url: &str, number: u32, is_movie: bool) -> String {
    let base = anime_url.trim().trim_end_matches('/');
    if is_movie {
        format!("{base}/pelicula/")
    } else {
        format!("{base}/{number}/")
    }
}

/// Ruta tras `jkplayer/` en la página del episodio (`um?e=...`).
pub fn extract_jkplayer_path(html: &str) -> Option<String> {
    Regex::new(r#"jkplayer/(um[^"'<> ]*)"#)
        .unwrap()
        .captures(html)
        .map(|c| c[1].to_string())
}

/// Parte tras `/jk.php` en el `<iframe>` del episodio (método legacy).
pub fn extract_jkphp_path(html: &str) -> Option<String> {
    Regex::new(r#"<iframe[^>]+src="/jk\.php([^"]*)""#)
        .unwrap()
        .captures(html)
        .map(|c| c[1].to_string())
}

/// URL del vídeo dentro del bloque `video: { url: '...' }` del reproductor.
/// Admite `url: 'https://...'` y `url: atob('...')`.
pub fn extract_player_video_url(html: &str) -> Option<String> {
    let start = html.find("video: {")?;
    let block = &html[start..html.len().min(start + 2000)];
    let caps = Regex::new(r#"url:\s*(?:atob\(\s*)?'([^']+)'"#)
        .unwrap()
        .captures(block)?;
    let raw = caps[1].trim();
    if raw.starts_with("http") {
        return Some(raw.to_string());
    }
    // Base64 (atob)
    let decoded = STANDARD.decode(raw).ok()?;
    let s = String::from_utf8(decoded).ok()?;
    s.starts_with("http").then(|| s.trim().to_string())
}

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    pub server: String,
    /// Enlace en Base64.
    pub remote: Option<String>,
}

/// Array de `var servers = [...];` de la página del episodio.
pub fn extract_servers(html: &str) -> Vec<Server> {
    Regex::new(r"(?s)var\s+servers\s*=\s*(\[.*?\]);")
        .unwrap()
        .captures(html)
        .and_then(|c| serde_json::from_str(&c[1]).ok())
        .unwrap_or_default()
}

pub fn mediafire_page_url(servers: &[Server]) -> Option<String> {
    servers
        .iter()
        .find(|s| s.server.eq_ignore_ascii_case("mediafire"))
        .and_then(|s| s.remote.as_deref())
        .and_then(|r| STANDARD.decode(r.trim()).ok())
        .and_then(|b| String::from_utf8(b).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.starts_with("http"))
}

/// Enlace real de descarga en la página de Mediafire (`https://download...`).
pub fn extract_mediafire_download(html: &str) -> Option<String> {
    Regex::new(r#"href="(https://download[^"]+)""#)
        .unwrap()
        .captures(html)
        .or_else(|| {
            Regex::new(r#"(https://download[^"' ]+)"#)
                .unwrap()
                .captures(html)
        })
        .map(|c| c[1].to_string())
}

/// Busca la fuente de vídeo de un episodio probando los tres métodos en orden.
pub async fn fetch_video_source(
    client: &JkClient,
    anime_url: &str,
    number: u32,
    is_movie: bool,
) -> Result<VideoSource> {
    let ep_url = episode_url(anime_url, number, is_movie);
    let html = client.get_text(&ep_url, None).await?;

    if let Some(path) = extract_jkplayer_path(&html) {
        let player = client
            .get_text(&format!("{BASE_URL}/jkplayer/{path}"), Some(&ep_url))
            .await;
        if let Some(url) = player.ok().as_deref().and_then(extract_player_video_url) {
            return Ok(VideoSource {
                url,
                kind: SourceKind::Jkplayer,
                referer: ep_url,
            });
        }
    }

    if let Some(path) = extract_jkphp_path(&html) {
        let player = client
            .get_text(&format!("{BASE_URL}/jk.php{path}"), Some(&ep_url))
            .await;
        if let Some(url) = player.ok().as_deref().and_then(extract_player_video_url) {
            return Ok(VideoSource {
                url,
                kind: SourceKind::Jkphp,
                referer: ep_url,
            });
        }
    }

    if let Some(mf) = mediafire_page_url(&extract_servers(&html))
        && let Ok(page) = client.get_text(&mf, None).await
        && let Some(url) = extract_mediafire_download(&page)
    {
        return Ok(VideoSource {
            url,
            kind: SourceKind::Mediafire,
            referer: mf,
        });
    }

    Err(JkError::SiteChanged(format!(
        "no se encontró ninguna fuente de vídeo en {ep_url}"
    )))
}
