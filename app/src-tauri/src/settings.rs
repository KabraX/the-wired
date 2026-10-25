//! Ajustes de la app, guardados como JSON en la carpeta de configuración de Tauri.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::library;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Carpeta raíz de la biblioteca de anime (la de Jellyfin).
    pub library_dir: PathBuf,
    /// Descargas a la vez (episodios). Los cambios se aplican al reiniciar la app.
    pub concurrent_downloads: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { library_dir: library::default_library_dir(), concurrent_downloads: 2 }
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

    pub fn sanitized(mut self) -> Self {
        self.concurrent_downloads = self.concurrent_downloads.clamp(1, 6);
        self
    }
}
