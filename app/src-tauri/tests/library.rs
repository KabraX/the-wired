//! Lectura de la biblioteca local (carpetas de descarga) con carpetas temporales.

use std::fs;
use std::path::PathBuf;

use the_wired_lib::catalog::{self, Index, IndexEntry};
use the_wired_lib::jkanime::Site;

fn temp_library(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("the-wired-lib-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn touch(path: PathBuf, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![0u8; bytes]).unwrap();
}

#[test]
fn episode_numbers_from_file_names() {
    let p = |f: &str| catalog::parse_episode(f, "Akira");
    assert_eq!(p("Claymore - S01E17.mp4"), Some((1, 17)));
    assert_eq!(p("Claymore - S02E05.mp4.part-Frag234.part"), Some((2, 5)));
    assert_eq!(p("Claymore - S01E04.mp4.ytdl"), Some((1, 4)));
    assert_eq!(p("Elfen Lied - 03.mp4"), Some((1, 3)));
    assert_eq!(p("Akira.mp4"), Some((1, 1)));
    assert_eq!(p("cover.jpg"), None);
    assert_eq!(p("notas.txt"), None);
}

#[test]
fn scan_counts_complete_partial_and_seasons() {
    let lib = temp_library("scan");
    // Jellyfin: dos temporadas, un episodio con restos al lado (a medias)
    let s1 = lib.join("Claymore/Season 01");
    touch(s1.join("Claymore - S01E01.mp4"), 1000);
    touch(s1.join("Claymore - S01E02.mp4"), 1000);
    touch(s1.join("Claymore - S01E02.mp4.part-Frag9.part"), 10);
    touch(lib.join("Claymore/Season 02/Claymore - S02E01.mp4"), 500);
    // Formato simple y una descarga que solo dejó restos
    touch(lib.join("Elfen Lied/Elfen Lied - 01.mp4"), 300);
    touch(lib.join("Takopii/Season 01/Takopii - S01E01.mp4.part"), 10);
    // Ni carpetas vacías ni ocultas
    fs::create_dir_all(lib.join("Vacía")).unwrap();
    touch(lib.join(".Trash/x - S01E01.mp4"), 10);

    let index = Index::load(None);
    index.record(IndexEntry {
        source: Site::Jkanime,
        folder: "Claymore".into(),
        title: "Claymore".into(),
        url: "https://jkanime.net/claymore/".into(),
        image: Some("https://cdn.example/claymore.jpg".into()),
    });
    let mut list = catalog::scan(Site::Jkanime, &lib, &index);
    list.sort_by(|a, b| a.name.cmp(&b.name));
    let names: Vec<_> = list.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["Claymore", "Elfen Lied", "Takopii"]);

    let c = &list[0];
    assert_eq!(
        (c.episodes, c.partial),
        (2, 1),
        "E02 tiene restos: cuenta como a medias"
    );
    assert_eq!(c.seasons, [1, 2]);
    assert_eq!(c.bytes, 2500);
    assert_eq!(c.url.as_deref(), Some("https://jkanime.net/claymore/"));
    assert!(c.image.is_some());

    assert_eq!((list[1].episodes, list[1].partial), (1, 0));
    assert!(
        list[1].url.is_none(),
        "sin entrada en el índice (bajado con el script)"
    );
    assert_eq!((list[2].episodes, list[2].partial), (0, 1));
    let _ = fs::remove_dir_all(&lib);
}

#[test]
fn index_keeps_the_poster_and_survives_restarts() {
    let dir = temp_library("index");
    let path = dir.join("library.json");
    let entry = |image: Option<&str>| IndexEntry {
        source: Site::Hentaijk,
        folder: "Bible Black".into(),
        title: "Bible Black".into(),
        url: "https://hentaijk.com/bible-black/".into(),
        image: image.map(str::to_string),
    };
    Index::load(Some(path.clone())).record(entry(Some("https://cdn.example/bb.jpg")));
    // Una entrada sin portada no borra la que había
    Index::load(Some(path.clone())).record(entry(None));
    let again = Index::load(Some(path.clone()));
    let e = again
        .get(Site::Hentaijk, "Bible Black")
        .expect("guardado en disco");
    assert_eq!(e.image.as_deref(), Some("https://cdn.example/bb.jpg"));
    assert!(
        again.get(Site::Jkanime, "Bible Black").is_none(),
        "cada fuente por separado"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn folder_names_to_search_queries() {
    assert_eq!(catalog::folder_query("Monster (2004)"), "Monster");
    assert_eq!(
        catalog::folder_query("Parasyte - Kiseijuu Sei no Kakuritsu (2014)"),
        "Parasyte - Kiseijuu Sei no Kakuritsu"
    );
    assert_eq!(catalog::folder_query("Elfen Lied"), "Elfen Lied");
    assert_eq!(
        catalog::normalize_title("Re:Zero kara"),
        catalog::normalize_title("re zero KARA")
    );
    assert_ne!(
        catalog::normalize_title("Another"),
        catalog::normalize_title("Another Season")
    );
}

#[test]
fn shorter_queries_when_the_folder_lost_punctuation() {
    assert_eq!(
        catalog::search_queries("Anehame Ore no Hatsukoi ga Jisshi na Wake ga Nai"),
        [
            "Anehame Ore no Hatsukoi ga Jisshi na Wake ga Nai",
            "Anehame Ore no",
            "Anehame"
        ]
    );
    assert_eq!(catalog::search_queries("Claymore"), ["Claymore"]);
    assert_eq!(
        catalog::search_queries("Re Zero kara Hajimeru"),
        ["Re Zero kara Hajimeru", "Re Zero kara"]
    );
    // Sin signos, el título de la web y la carpeta coinciden
    assert_eq!(
        catalog::normalize_title("Anehame: Ore no Hatsukoi ga Jisshi na Wake ga Nai"),
        catalog::normalize_title("Anehame Ore no Hatsukoi ga Jisshi na Wake ga Nai")
    );
}
