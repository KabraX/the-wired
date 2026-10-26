//! Ajustes de la app, guardados como JSON en la carpeta de configuración de Tauri.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::jkanime::Site;
use crate::library;

/// Ajustes de cada sitio: cada uno tiene su propia biblioteca.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceSettings {
    /// jkanime siempre está activo; el resto se activa en Ajustes.
    pub enabled: bool,
    /// Carpeta raíz de la biblioteca (la de Jellyfin).
    pub library_dir: PathBuf,
    /// Carpetas `Season NN` y nombres `S01E01` (formato Jellyfin). Si no, todo en la
    /// carpeta del anime.
    pub jellyfin_layout: bool,
}

impl Default for SourceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            library_dir: library::default_library_dir(),
            jellyfin_layout: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Descargas a la vez (episodios). Los cambios se aplican al reiniciar la app.
    pub concurrent_downloads: u32,
    pub sources: BTreeMap<Site, SourceSettings>,
    // Formato de la 1.1 (solo jkanime): se lee para migrarlo a `sources` y no se guarda.
    #[serde(skip_serializing)]
    library_dir: Option<PathBuf>,
    #[serde(skip_serializing)]
    jellyfin_layout: Option<bool>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            concurrent_downloads: 2,
            sources: BTreeMap::new(),
            library_dir: None,
            jellyfin_layout: None,
        }
    }
}

impl Settings {
    pub fn load(path: &std::path::Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self).unwrap_or_default())
    }

    /// Corrige valores fuera de rango y completa los sitios que falten.
    pub fn sanitized(mut self) -> Self {
        self.concurrent_downloads = self.concurrent_downloads.clamp(1, 6);

        let jk = self.sources.entry(Site::Jkanime).or_default();
        if let Some(dir) = self.library_dir.take() {
            jk.library_dir = dir;
        }
        if let Some(j) = self.jellyfin_layout.take() {
            jk.jellyfin_layout = j;
        }
        jk.enabled = true;
        let jk_dir = jk.library_dir.clone();

        // hentaijk: por defecto en una carpeta hermana ("…/Anime" → "…/Hentai"),
        // para que no se mezcle con la biblioteca de anime
        self.sources
            .entry(Site::Hentaijk)
            .or_insert_with(|| SourceSettings {
                library_dir: jk_dir
                    .parent()
                    .map(|p| p.join("Hentai"))
                    .unwrap_or_else(|| jk_dir.join("Hentai")),
                ..Default::default()
            });
        self
    }

    pub fn source(&self, site: Site) -> SourceSettings {
        self.sources.get(&site).cloned().unwrap_or_default()
    }

    /// Carpeta y formato con los que se guarda lo de un sitio.
    pub fn library(&self, site: Site) -> (PathBuf, library::Layout) {
        let s = self.source(site);
        (
            s.library_dir,
            library::Layout::from_jellyfin(s.jellyfin_layout),
        )
    }
}
