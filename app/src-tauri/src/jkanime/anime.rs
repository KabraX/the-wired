//! Ficha del anime: datos, token CSRF y lista de episodios por su API AJAX.

use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};

use super::{JkClient, JkError, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Episode {
    pub number: u32,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnimeInfo {
    /// URL de la ficha, sin barra final (`https://jkanime.net/elfen-lied`).
    pub url: String,
    pub title: String,
    pub image: Option<String>,
    /// "Serie", "Pelicula", "OVA"…
    pub kind: Option<String>,
    /// "1080p", "720p"… según la ficha (no todas lo indican).
    pub quality: Option<String>,
    pub is_movie: bool,
    pub episodes: Vec<Episode>,
}

/// Lo que se extrae del HTML de la ficha antes de llamar a la API.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimePage {
    pub title: Option<String>,
    pub image: Option<String>,
    pub kind: Option<String>,
    pub quality: Option<String>,
    pub csrf: Option<String>,
    pub episodes_api: Option<String>,
    /// "Episodios: 13" de la ficha, como respaldo si la API falla.
    pub declared_episodes: Option<u32>,
}

pub fn parse_anime_page(html: &str) -> AnimePage {
    let doc = Html::parse_document(html);
    let meta = |sel: &str| {
        Selector::parse(sel).ok().and_then(|s| {
            doc.select(&s)
                .next()
                .and_then(|e| e.value().attr("content").map(str::to_string))
        })
    };
    let csrf = meta(r#"meta[name="csrf-token"]"#);
    let image = meta(r#"meta[property="og:image"]"#);
    // og:title es "Elfen Lied - anime Elfen Lied online JkAnime": nos quedamos con lo de antes de " - "
    let title = meta(r#"meta[property="og:title"]"#)
        .map(|t| t.split(" - ").next().unwrap_or(&t).trim().to_string())
        .filter(|t| !t.is_empty());

    let episodes_api = Regex::new(r"url:\s*'(https://jkanime\.net/ajax/episodes/\d+/?)'")
        .unwrap()
        .captures(html)
        .map(|c| c[1].to_string());
    let kind = Regex::new(r"<span>Tipo:</span>\s*([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .filter(|k| !k.is_empty());
    let quality = Regex::new(r"<span>Calidad:</span>\s*([^<]+)")
        .unwrap()
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .filter(|q| !q.is_empty());
    let declared_episodes = Regex::new(r"<span>Episodios:</span>\s*(\d+)")
        .unwrap()
        .captures(html)
        .and_then(|c| c[1].parse().ok());

    AnimePage {
        title,
        image,
        kind,
        quality,
        csrf,
        episodes_api,
        declared_episodes,
    }
}

/// Respuesta paginada de `ajax/episodes/<id>/` (formato Laravel).
#[derive(Debug, Deserialize)]
pub struct EpisodesResponse {
    pub data: Vec<EpisodeItem>,
    pub total: u32,
    pub current_page: u32,
    pub last_page: u32,
}

/// `https://jkanime.net/ajax/episodes/123` + página 2 → `.../ajax/episodes/123/2`
pub fn episodes_page_url(api: &str, page: u32) -> String {
    format!("{}/{page}", api.trim_end_matches('/'))
}

#[derive(Debug, Deserialize)]
pub struct EpisodeItem {
    pub number: u32,
    pub title: Option<String>,
}

pub fn parse_episodes_json(json: &str) -> Result<EpisodesResponse> {
    serde_json::from_str(json)
        .map_err(|e| JkError::SiteChanged(format!("respuesta de episodios inesperada: {e}")))
}

pub fn is_movie_kind(kind: Option<&str>) -> bool {
    kind.map(|k| k.eq_ignore_ascii_case("pelicula") || k.eq_ignore_ascii_case("película"))
        .unwrap_or(false)
}

/// Descarga la ficha y la lista completa de episodios.
pub async fn fetch_anime(client: &JkClient, url: &str) -> Result<AnimeInfo> {
    let url = url.trim().trim_end_matches('/').to_string();
    let page_url = format!("{url}/");
    let html = client.get_text(&page_url, None).await?;
    let page = parse_anime_page(&html);

    let title = page
        .title
        .clone()
        .ok_or_else(|| JkError::SiteChanged("la ficha no tiene título".into()))?;
    let is_movie = is_movie_kind(page.kind.as_deref());

    let mut episodes = Vec::new();
    if let (Some(api), Some(csrf)) = (&page.episodes_api, &page.csrf) {
        let api = api.trim_end_matches('/');
        let mut p = 1;
        loop {
            // La web pide las páginas como `.../ajax/episodes/<id>/<página>` (16 por página)
            let json = client
                .post_ajax(&episodes_page_url(api, p), csrf, &page_url)
                .await?;
            let resp = parse_episodes_json(&json)?;
            if resp.current_page != p {
                return Err(JkError::SiteChanged(format!(
                    "la API de episodios devolvió la página {} al pedir la {p}",
                    resp.current_page
                )));
            }
            episodes.extend(resp.data.into_iter().map(|e| Episode {
                number: e.number,
                title: e.title.unwrap_or_else(|| format!("{title} - {}", e.number)),
            }));
            if p >= resp.last_page {
                break;
            }
            p += 1;
        }
    }

    // Respaldo: si la API no dio nada pero la ficha declara episodios, numerarlos.
    if episodes.is_empty() {
        let n = page
            .declared_episodes
            .unwrap_or(if is_movie { 1 } else { 0 });
        if n == 0 {
            return Err(JkError::SiteChanged(
                "no se encontró la lista de episodios".into(),
            ));
        }
        episodes = (1..=n)
            .map(|i| Episode {
                number: i,
                title: format!("{title} - {i}"),
            })
            .collect();
    }
    episodes.sort_by_key(|e| e.number);
    episodes.dedup_by_key(|e| e.number);

    Ok(AnimeInfo {
        url,
        title,
        image: page.image,
        kind: page.kind,
        quality: page.quality,
        is_movie,
        episodes,
    })
}
