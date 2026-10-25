//! Tests del scraper contra páginas recortadas de jkanime (tests/fixtures), sin red.
//! Si jkanime cambia su web, se guarda una página nueva, se recorta y se ajustan estos tests.
//!
//! Prueba real contra la web (lenta, depende de que jkanime responda):
//!   cargo test --test scraper -- --ignored --nocapture

use std::path::Path;

use the_wired_lib::jkanime::{anime, episode, search, JkClient};
use the_wired_lib::library;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

// ---------------------------------------------------------------- búsqueda

#[test]
fn search_parses_cards() {
    let r = search::parse_search(&fixture("search.html"));
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].title, "Elfen Lied");
    assert_eq!(r[0].url, "https://jkanime.net/elfen-lied/");
    assert_eq!(r[0].image.as_deref(), Some("https://cdn.example/elfen-lied.jpg"));
    assert_eq!(r[0].kind.as_deref(), Some("Serie"));
    assert_eq!(r[0].status.as_deref(), Some("Concluido"));
    // Las entidades HTML se decodifican (el script quitaba &quot; a mano)
    assert_eq!(r[1].title, "Elfen Lied: \"Regenbogen\"");
    assert_eq!(r[1].kind.as_deref(), Some("OVA"));
}

#[test]
fn search_empty_page() {
    assert!(search::parse_search(&fixture("search-empty.html")).is_empty());
}

#[test]
fn search_urls_like_the_script() {
    assert_eq!(search::query_path("  elfen   lied "), "elfen_lied");
    assert_eq!(search::search_url("elfen lied", 1), "https://jkanime.net/buscar/elfen_lied/");
    assert_eq!(search::search_url("elfen lied", 2), "https://jkanime.net/buscar/elfen_lied/2/");
}

// ---------------------------------------------------------------- ficha

#[test]
fn anime_page_fields() {
    let p = anime::parse_anime_page(&fixture("anime.html"));
    assert_eq!(p.title.as_deref(), Some("Elfen Lied"));
    assert_eq!(p.kind.as_deref(), Some("Serie"));
    assert_eq!(p.csrf.as_deref(), Some("TOKEN_FALSO"));
    assert_eq!(p.episodes_api.as_deref(), Some("https://jkanime.net/ajax/episodes/270/"));
    assert_eq!(p.declared_episodes, Some(13));
    assert_eq!(p.image.as_deref(), Some("https://cdn.example/elfen-lied.jpg"));
}

#[test]
fn episodes_json() {
    let r = anime::parse_episodes_json(&fixture("episodes.json")).unwrap();
    assert_eq!(r.total, 2);
    assert_eq!(r.last_page, 1);
    assert_eq!(r.current_page, 1);
    assert_eq!(r.data.iter().map(|e| e.number).collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn episodes_pages_go_in_the_path() {
    // La web usa '.../ajax/episodes/123/' + pag; con '?p=2' devuelve siempre la página 1
    assert_eq!(anime::episodes_page_url("https://jkanime.net/ajax/episodes/123/", 2), "https://jkanime.net/ajax/episodes/123/2");
    assert_eq!(anime::episodes_page_url("https://jkanime.net/ajax/episodes/123", 1), "https://jkanime.net/ajax/episodes/123/1");
}

#[test]
fn movie_kind() {
    assert!(anime::is_movie_kind(Some("Pelicula")));
    assert!(anime::is_movie_kind(Some("película")));
    assert!(!anime::is_movie_kind(Some("Serie")));
    assert!(!anime::is_movie_kind(None));
}

// ---------------------------------------------------------------- episodio

#[test]
fn episode_urls() {
    assert_eq!(episode::episode_url("https://jkanime.net/elfen-lied/", 3, false), "https://jkanime.net/elfen-lied/3/");
    assert_eq!(episode::episode_url("https://jkanime.net/akira", 1, true), "https://jkanime.net/akira/pelicula/");
}

#[test]
fn jkplayer_chain() {
    let ep = fixture("episode.html");
    assert_eq!(
        episode::extract_jkplayer_path(&ep).as_deref(),
        Some("um?e=FALSO123&t=abc&op=NzQ2MDI=")
    );
    assert_eq!(
        episode::extract_player_video_url(&fixture("jkplayer.html")).as_deref(),
        Some("https://cdn.example/elfen-lied/FALSO.m3u8?st=abc&e=1792981839")
    );
}

#[test]
fn player_url_in_base64() {
    assert_eq!(
        episode::extract_player_video_url(&fixture("jkplayer-atob.html")).as_deref(),
        Some("https://cdn.example/atob/FALSO.m3u8")
    );
}

#[test]
fn jkphp_legacy() {
    assert_eq!(
        episode::extract_jkphp_path(&fixture("episode-legacy.html")).as_deref(),
        Some("?u=stream/jkmedia/FALSO")
    );
    assert!(episode::extract_jkplayer_path(&fixture("episode-legacy.html")).is_none());
}

#[test]
fn mediafire_fallback() {
    let servers = episode::extract_servers(&fixture("episode.html"));
    assert_eq!(servers.len(), 2);
    assert_eq!(
        episode::mediafire_page_url(&servers).as_deref(),
        Some("https://www.mediafire.com/file/falso/ep1.mp4")
    );
    assert_eq!(
        episode::extract_mediafire_download(&fixture("mediafire.html")).as_deref(),
        Some("https://download1234.mediafire.com/falso/ep1.mp4")
    );
}

// ---------------------------------------------------------------- biblioteca

#[test]
fn library_names_match_the_script() {
    assert_eq!(library::sanitize_filename("Re:Zero <Kara> \"Hajimeru\"? ..."), "ReZero Kara Hajimeru");
    assert_eq!(library::sanitize_filename("Shingeki no Kyojin"), "Shingeki no Kyojin");
    assert_eq!(library::sanitize_filename("Pokémon: ¿Quién es?"), "Pokémon ¿Quién es");
    assert_eq!(library::episode_filename("Elfen Lied", 1, 2), "Elfen Lied - S01E02.mp4");
    let p = library::episode_path(Path::new("/lib/Anime"), "Re:Zero", 2, 15);
    assert_eq!(p, Path::new("/lib/Anime/ReZero/Season 02/ReZero - S02E15.mp4"));
}

// ---------------------------------------------------------------- red (manual)

#[tokio::test]
#[ignore = "usa la red y depende de que jkanime responda"]
async fn live_search_anime_and_source() {
    let client = JkClient::new();
    let results = search::search(&client, "elfen lied", 1).await.expect("búsqueda");
    let first = results.iter().find(|r| r.title == "Elfen Lied").expect("Elfen Lied en resultados");
    println!("búsqueda: {} resultados, primero {:?}", results.len(), first.url);

    let info = anime::fetch_anime(&client, &first.url).await.expect("ficha");
    println!("ficha: {} ({:?}), {} episodios", info.title, info.kind, info.episodes.len());
    assert!(!info.episodes.is_empty());

    let src = episode::fetch_video_source(&client, &info.url, 1, info.is_movie).await.expect("fuente");
    println!("fuente: {:?} {}", src.kind, &src.url[..src.url.len().min(60)]);
    assert!(src.url.starts_with("http"));
}

#[tokio::test]
#[ignore = "usa la red y depende de que jkanime responda"]
async fn live_paginated_episodes() {
    // Más de 16 episodios: la API los devuelve en varias páginas
    let client = JkClient::new();
    let info = anime::fetch_anime(&client, "https://jkanime.net/naruto/").await.expect("ficha");
    println!("{}: {} episodios, último {:?}", info.title, info.episodes.len(), info.episodes.last().map(|e| e.number));
    assert!(info.episodes.len() > 16);
    let numbers: Vec<u32> = info.episodes.iter().map(|e| e.number).collect();
    assert_eq!(numbers, (1..=numbers.len() as u32).collect::<Vec<_>>(), "episodios consecutivos sin huecos");
}
