//! Icono de la bandeja del sistema: progreso global y menú (mostrar, pausar, salir).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::download::{self, DownloadManager, Job, Status};

/// En Linux la bandeja necesita AppIndicator; si falta, Tauri aborta al crear el icono.
/// Se comprueba antes para que la app funcione igual sin bandeja.
pub fn available() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    let names = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"];
    let mut dirs: Vec<std::path::PathBuf> = [
        "/usr/lib",
        "/usr/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib/x86_64-linux-gnu",
    ]
    .iter()
    .map(std::path::PathBuf::from)
    .collect();
    // AppImage: las librerías van dentro de la imagen
    if let Some(appdir) = std::env::var_os("APPDIR") {
        let appdir = std::path::PathBuf::from(appdir);
        dirs.push(appdir.join("usr/lib"));
        dirs.push(appdir.join("usr/lib/x86_64-linux-gnu"));
    }
    if let Some(ld) = std::env::var_os("LD_LIBRARY_PATH") {
        dirs.extend(std::env::split_paths(&ld));
    }
    dirs.iter()
        .any(|d| names.iter().any(|n| d.join(n).exists()))
}

/// Id del icono, para recuperarlo con `AppHandle::tray_by_id`.
pub const ID: &str = "main";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<TrayIcon<R>> {
    let show = MenuItem::with_id(app, "show", "Mostrar the-wired", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause_all", "Pausar descargas", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume_all", "Reanudar descargas", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &pause, &resume, &quit])?;

    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("the-wired")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "pause_all" => app.state::<DownloadManager>().pause_all(app),
            "resume_all" => app.state::<DownloadManager>().resume_all(app),
            // La cola está guardada: lo pendiente se retoma al volver a abrir
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

fn show_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Texto del icono según la cola: "2 descargando · 45 %", "3 en cola", "En pausa"…
pub fn summary(jobs: &[Job]) -> String {
    let downloading: Vec<f64> = jobs
        .iter()
        .filter_map(|j| match j.status {
            Status::Downloading { percent, .. } => Some(percent.unwrap_or(0.0)),
            Status::Resolving => Some(0.0),
            _ => None,
        })
        .collect();
    let queued = jobs
        .iter()
        .filter(|j| matches!(j.status, Status::Queued | Status::Waiting { .. }))
        .count();
    let paused = jobs
        .iter()
        .filter(|j| matches!(j.status, Status::Paused))
        .count();
    if !downloading.is_empty() {
        let avg = downloading.iter().sum::<f64>() / downloading.len() as f64;
        let mut s = format!("{} descargando · {:.0} %", downloading.len(), avg);
        if queued > 0 {
            s.push_str(&format!(" · {queued} en cola"));
        }
        s
    } else if queued > 0 {
        format!("{queued} en cola")
    } else if paused > 0 {
        format!("{paused} en pausa")
    } else {
        "Sin descargas".into()
    }
}

pub fn update<R: Runtime>(tray: &TrayIcon<R>, jobs: &[Job]) {
    let text = format!("the-wired — {}", summary(jobs));
    let _ = tray.set_tooltip(Some(&text));
    // En Linux (AppIndicator) el tooltip no se muestra; el título sí, junto al icono
    #[cfg(target_os = "linux")]
    let _ = tray.set_title(
        jobs.iter()
            .any(|j| j.status.is_active())
            .then(|| summary(jobs)),
    );
}

/// Aviso (una vez por sesión) de que la app sigue trabajando en segundo plano.
pub fn notify_background<R: Runtime>(app: &AppHandle<R>) {
    static SHOWN: AtomicBool = AtomicBool::new(false);
    if !SHOWN.swap(true, Ordering::Relaxed) {
        download::notify(
            app,
            "the-wired sigue descargando",
            "La ventana se ha ocultado. Ábrela o sal desde el icono de la bandeja.",
        );
    }
}
