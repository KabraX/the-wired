pub mod jkanime;
pub mod library;

use jkanime::{anime::AnimeInfo, episode::VideoSource, search::SearchResult, JkClient, JkError};
use serde::Serialize;

/// Datos básicos de la app, para comprobar que la interfaz y Rust se comunican.
#[derive(Serialize)]
struct AppInfo {
    version: &'static str,
    os: &'static str,
    arch: &'static str,
    library_dir: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        library_dir: library::default_library_dir().display().to_string(),
    }
}

#[tauri::command]
async fn search(
    client: tauri::State<'_, JkClient>,
    query: String,
    page: Option<u32>,
) -> Result<Vec<SearchResult>, JkError> {
    jkanime::search::search(&client, &query, page.unwrap_or(1)).await
}

#[tauri::command]
async fn anime(client: tauri::State<'_, JkClient>, url: String) -> Result<AnimeInfo, JkError> {
    jkanime::anime::fetch_anime(&client, &url).await
}

#[tauri::command]
async fn video_source(
    client: tauri::State<'_, JkClient>,
    anime_url: String,
    number: u32,
    is_movie: bool,
) -> Result<VideoSource, JkError> {
    jkanime::episode::fetch_video_source(&client, &anime_url, number, is_movie).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(JkClient::new())
        .invoke_handler(tauri::generate_handler![app_info, search, anime, video_source])
        .run(tauri::generate_context!())
        .expect("error al arrancar the-wired");
}
