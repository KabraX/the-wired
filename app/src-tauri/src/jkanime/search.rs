//! Búsqueda: `https://jkanime.net/buscar/<consulta>/` (y `/<página>/` para las siguientes).

use scraper::{Html, Selector};
use serde::Serialize;

use super::{BASE_URL, JkClient, JkError, Result};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchResult {
    pub title: String,
    /// Enlace a la ficha, p. ej. `https://jkanime.net/elfen-lied/`.
    pub url: String,
    pub image: Option<String>,
    /// "Serie", "Pelicula", "OVA"…
    pub kind: Option<String>,
    /// "Concluido", "En emision"…
    pub status: Option<String>,
}

/// Convierte la consulta como el script: espacios a `_`.
pub fn query_path(query: &str) -> String {
    query.split_whitespace().collect::<Vec<_>>().join("_")
}

pub fn search_url(query: &str, page: u32) -> String {
    let q = url::form_urlencoded::byte_serialize(query_path(query).as_bytes()).collect::<String>();
    if page <= 1 {
        format!("{BASE_URL}/buscar/{q}/")
    } else {
        format!("{BASE_URL}/buscar/{q}/{page}/")
    }
}

/// Extrae las tarjetas de resultados (`div.anime__item`).
pub fn parse_search(html: &str) -> Vec<SearchResult> {
    let doc = Html::parse_document(html);
    let item = Selector::parse("div.anime__item").unwrap();
    let link = Selector::parse("h5 > a").unwrap();
    let pic = Selector::parse(".anime__item__pic").unwrap();
    let li = Selector::parse(".anime__item__text li").unwrap();
    let kind_li = Selector::parse(".anime__item__text li.anime").unwrap();

    doc.select(&item)
        .filter_map(|card| {
            let a = card.select(&link).next()?;
            let title = text_of(&a)?;
            let url = a.value().attr("href")?.trim().to_string();
            let image = card
                .select(&pic)
                .next()
                .and_then(|p| p.value().attr("data-setbg"))
                .map(|s| s.trim().to_string());
            let kind = card.select(&kind_li).next().and_then(|e| text_of(&e));
            let status = card
                .select(&li)
                .find(|e| !e.value().classes().any(|c| c == "anime"))
                .and_then(|e| text_of(&e));
            Some(SearchResult {
                title,
                url,
                image,
                kind,
                status,
            })
        })
        .collect()
}

fn text_of(e: &scraper::ElementRef) -> Option<String> {
    let t = e
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!t.is_empty()).then_some(t)
}

/// Busca una página de resultados. Una página inexistente (404) devuelve lista vacía.
pub async fn search(client: &JkClient, query: &str, page: u32) -> Result<Vec<SearchResult>> {
    if query_path(query).is_empty() {
        return Ok(Vec::new());
    }
    match client.get_text(&search_url(query, page), None).await {
        Ok(html) => {
            let results = parse_search(&html);
            // Una página con contenido pero sin el contenedor de resultados ni el
            // formulario de búsqueda no es la página que esperamos.
            if results.is_empty() && !html.contains("anime__item") && !html.contains("buscar") {
                return Err(JkError::SiteChanged(
                    "la página de búsqueda no tiene el formato esperado".into(),
                ));
            }
            Ok(results)
        }
        Err(JkError::NotFound(_)) => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}
