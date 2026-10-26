//! Biblioteca local: qué animes hay en las carpetas de descarga de cada fuente.
//!
//! Las carpetas se leen tal cual (sirve también para lo bajado con el script). Aparte se
//! guarda un índice pequeño en los datos de la app (`library.json`) con el enlace y la
//! portada de cada anime, para poder abrir su ficha desde la biblioteca. La carpeta de
//! la biblioteca no se toca: Jellyfin no ve nada nuevo.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::jkanime::Site;
use crate::library;

/// Lo que se sabe de un anime por haberlo descargado desde la app.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub source: Site,
    /// Nombre de la carpeta (el de la serie con formato Jellyfin).
    pub folder: String,
    /// Título en el sitio (puede diferir de la carpeta: "Overlord II" → "Overlord").
    pub title: String,
    pub url: String,
    pub image: Option<String>,
}

/// Índice `fuente/carpeta` → datos, guardado como JSON.
pub struct Index {
    path: Option<PathBuf>,
    entries: Mutex<BTreeMap<String, IndexEntry>>,
}

fn key(source: Site, folder: &str) -> String {
    format!(
        "{}/{}",
        serde_json::to_value(source)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
        library::sanitize_filename(folder)
    )
}

impl Index {
    pub fn load(path: Option<PathBuf>) -> Self {
        let entries = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        Self {
            path,
            entries: Mutex::new(entries),
        }
    }

    /// Añade o actualiza un anime. Una entrada sin portada no borra la que ya había.
    pub fn record(&self, entry: IndexEntry) {
        let mut entries = self.entries.lock().unwrap();
        let k = key(entry.source, &entry.folder);
        let image = entry
            .image
            .clone()
            .or_else(|| entries.get(&k).and_then(|e| e.image.clone()));
        let changed = entries.get(&k).is_none_or(|old| {
            old.url != entry.url || old.title != entry.title || old.image != image
        });
        entries.insert(k, IndexEntry { image, ..entry });
        if changed && let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let tmp = path.with_extension("json.tmp");
            let ok = serde_json::to_vec_pretty(&*entries)
                .ok()
                .is_some_and(|data| std::fs::write(&tmp, data).is_ok());
            if !ok || std::fs::rename(&tmp, path).is_err() {
                log::warn!(
                    "no se pudo guardar el índice de la biblioteca en {}",
                    path.display()
                );
            }
        }
    }

    pub fn get(&self, source: Site, folder: &str) -> Option<IndexEntry> {
        self.entries
            .lock()
            .unwrap()
            .get(&key(source, folder))
            .cloned()
    }
}

/// Título comparable: minúsculas, solo letras y números ("Re:Zero" ≈ "rezero").
pub fn normalize_title(t: &str) -> String {
    t.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Nombre de carpeta sin el año que añaden otros programas: "Monster (2004)" → "Monster".
pub fn folder_query(name: &str) -> String {
    Regex::new(r"\s*\(\d{4}\)\s*$")
        .unwrap()
        .replace(name, "")
        .trim()
        .to_string()
}

/// Consultas para buscar una carpeta en la web, de la más precisa a la más amplia:
/// el nombre completo, las tres primeras palabras y la primera (si tiene 4+ letras,
/// para no buscar "No" o "Re").
pub fn search_queries(name: &str) -> Vec<String> {
    let words: Vec<&str> = name.split_whitespace().collect();
    let mut out = vec![words.join(" ")];
    for n in [3, 1] {
        if words.len() > n {
            let q = words[..n].join(" ");
            if (n > 1 || q.chars().count() >= 4) && !out.contains(&q) {
                out.push(q);
            }
        }
    }
    out
}

/// Un anime encontrado en la carpeta de la biblioteca.
#[derive(Debug, Clone, Serialize)]
pub struct LibraryAnime {
    pub source: Site,
    /// Nombre de la carpeta.
    pub name: String,
    pub path: String,
    /// Temporadas con algo dentro (con el formato simple, solo la 1).
    pub seasons: Vec<u32>,
    /// Episodios completos (MP4 sin restos de descarga).
    pub episodes: u32,
    /// Episodios con restos de una descarga a medias.
    pub partial: u32,
    pub bytes: u64,
    /// Última modificación (segundos Unix), para ordenar por lo más reciente.
    pub modified: u64,
    /// Del índice: enlace a la ficha y portada (si se descargó desde la app).
    pub title: Option<String>,
    pub url: Option<String>,
    pub image: Option<String>,
}

/// `(temporada, episodio)` del nombre de un archivo: `… - S01E05.mp4`, `… - 05.mp4`,
/// también con restos (`….mp4.part-Frag3.part`). Una película (`Akira.mp4`) es el 1×1.
pub fn parse_episode(file: &str, anime: &str) -> Option<(u32, u32)> {
    let re = Regex::new(r"(?i)S(\d{1,3})E(\d{1,4})\.mp4").unwrap();
    if let Some(c) = re.captures(file) {
        return Some((c[1].parse().ok()?, c[2].parse().ok()?));
    }
    let re = Regex::new(r"(?i) - (\d{1,4})\.mp4").unwrap();
    if let Some(c) = re.captures(file) {
        return Some((1, c[1].parse().ok()?));
    }
    let movie = format!("{}.mp4", library::sanitize_filename(anime));
    file.starts_with(&movie).then_some((1, 1))
}

fn is_partial(file: &str) -> bool {
    file.contains(".part") || file.ends_with(".ytdl")
}

/// Lee una carpeta de anime (y sus `Season NN`).
pub fn scan_anime(source: Site, dir: &Path) -> Option<LibraryAnime> {
    let name = dir.file_name()?.to_string_lossy().to_string();
    let season_re = Regex::new(r"^Season (\d+)$").unwrap();
    let mut done = BTreeSet::new();
    let mut partial = BTreeSet::new();
    let mut seasons = BTreeSet::new();
    let mut bytes = 0u64;
    let mut modified = 0u64;

    let mut visit = |path: &Path| {
        let Ok(entries) = std::fs::read_dir(path) else {
            return;
        };
        for e in entries.flatten() {
            let file = e.file_name().to_string_lossy().to_string();
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let Some(ep) = parse_episode(&file, &name) else {
                continue;
            };
            if let Ok(m) = meta.modified()
                && let Ok(d) = m.duration_since(std::time::UNIX_EPOCH)
            {
                modified = modified.max(d.as_secs());
            }
            if is_partial(&file) {
                partial.insert(ep);
            } else if file.to_ascii_lowercase().ends_with(".mp4") {
                bytes += meta.len();
                done.insert(ep);
                seasons.insert(ep.0);
            }
        }
    };

    visit(dir);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if season_re.is_match(&n) && e.path().is_dir() {
                visit(&e.path());
            }
        }
    }

    // Un MP4 con restos al lado está a medias, no completo
    let partial: BTreeSet<_> = partial.into_iter().collect();
    let complete = done.difference(&partial).count() as u32;
    if complete == 0 && partial.is_empty() {
        return None;
    }
    Some(LibraryAnime {
        source,
        path: dir.display().to_string(),
        name,
        seasons: seasons.into_iter().collect(),
        episodes: complete,
        partial: partial.len() as u32,
        bytes,
        modified,
        title: None,
        url: None,
        image: None,
    })
}

/// Todos los animes de una carpeta de biblioteca, lo más reciente primero.
pub fn scan(source: Site, library_dir: &Path, index: &Index) -> Vec<LibraryAnime> {
    let Ok(entries) = std::fs::read_dir(library_dir) else {
        return Vec::new();
    };
    let mut list: Vec<LibraryAnime> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| scan_anime(source, &e.path()))
        .map(|mut a| {
            if let Some(i) = index.get(source, &a.name) {
                a.title = Some(i.title);
                a.url = Some(i.url);
                a.image = i.image;
            }
            a
        })
        .collect();
    list.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name)));
    list
}
