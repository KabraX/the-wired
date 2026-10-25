//! Scraper de jkanime.net.
//!
//! Traducción a Rust de la lógica del script `the-wired`. Las funciones `parse_*` /
//! `extract_*` son puras (reciben HTML y devuelven datos) para poder probarlas con
//! páginas guardadas, sin red. Las funciones `async` hacen las peticiones.

pub mod anime;
pub mod episode;
pub mod search;

use std::time::Duration;

use reqwest::StatusCode;
use serde::Serialize;

pub const BASE_URL: &str = "https://jkanime.net";

/// Mismo navegador con el que se identifica el script.
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36";

/// Errores que la interfaz distingue para dar un mensaje claro.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail")]
pub enum JkError {
    /// No hay conexión, se agotó el tiempo o jkanime no responde (5xx tras reintentos).
    #[error("jkanime.net no responde: {0}")]
    Unavailable(String),
    /// La página no existe (404).
    #[error("no encontrado: {0}")]
    NotFound(String),
    /// La página llegó pero no tiene lo que esperamos: probablemente la web cambió.
    #[error("la web de jkanime parece haber cambiado: {0}")]
    SiteChanged(String),
}

pub type Result<T> = std::result::Result<T, JkError>;

/// Cliente HTTP con cookies (la API de episodios usa sesión + token CSRF)
/// y reintentos ante los 502/503/504 que jkanime devuelve a menudo.
#[derive(Clone)]
pub struct JkClient {
    http: reqwest::Client,
    retries: u32,
}

impl JkClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .cookie_store(true)
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .expect("configuración del cliente HTTP");
        Self { http, retries: 3 }
    }

    /// GET que devuelve el cuerpo como texto, con reintentos ante errores 5xx de red.
    pub async fn get_text(&self, url: &str, referer: Option<&str>) -> Result<String> {
        self.send_text(|| {
            let mut req = self.http.get(url);
            if let Some(r) = referer {
                req = req.header(reqwest::header::REFERER, r);
            }
            req
        }, url)
        .await
    }

    /// POST de formulario (API de episodios) con cabeceras de petición AJAX.
    pub async fn post_ajax(&self, url: &str, csrf: &str, referer: &str) -> Result<String> {
        self.send_text(|| {
            self.http
                .post(url)
                .header("X-Requested-With", "XMLHttpRequest")
                .header("X-CSRF-TOKEN", csrf)
                .header(reqwest::header::REFERER, referer)
                .form(&[("_token", csrf)])
        }, url)
        .await
    }

    async fn send_text(
        &self,
        build: impl Fn() -> reqwest::RequestBuilder,
        url: &str,
    ) -> Result<String> {
        let mut last = String::new();
        for attempt in 0..self.retries {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(800 * u64::from(attempt))).await;
            }
            match build().send().await {
                Ok(resp) => match resp.status() {
                    s if s.is_success() => {
                        return resp
                            .text()
                            .await
                            .map_err(|e| JkError::Unavailable(e.to_string()));
                    }
                    StatusCode::NOT_FOUND => return Err(JkError::NotFound(url.to_string())),
                    s if s.is_server_error() => last = format!("HTTP {} en {url}", s.as_u16()),
                    s => return Err(JkError::Unavailable(format!("HTTP {} en {url}", s.as_u16()))),
                },
                Err(e) => last = e.to_string(),
            }
        }
        Err(JkError::Unavailable(last))
    }
}

impl Default for JkClient {
    fn default() -> Self {
        Self::new()
    }
}

