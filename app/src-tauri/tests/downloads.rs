//! Pruebas del gestor de descargas con una app de Tauri simulada.
//!
//!   cargo test --test downloads                         (sin red)
//!   cargo test --test downloads -- --ignored --nocapture (descarga real unos segundos)

use std::path::PathBuf;
use std::time::{Duration, Instant};

use the_wired_lib::download::{DownloadManager, EpisodeRequest, Job, Status};
use the_wired_lib::jkanime::JkClient;
use the_wired_lib::library;

fn temp_library(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("the-wired-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn request(episode: u32) -> EpisodeRequest {
    EpisodeRequest {
        anime: "Elfen Lied".into(),
        anime_url: "https://jkanime.net/elfen-lied".into(),
        is_movie: false,
        season: 1,
        episode,
    }
}

async fn wait_for(dm: &DownloadManager, id: u64, secs: u64, pred: impl Fn(&Status) -> bool) -> Job {
    let start = Instant::now();
    loop {
        let job = dm.jobs().into_iter().find(|j| j.id == id).expect("trabajo");
        if pred(&job.status) || start.elapsed() > Duration::from_secs(secs) {
            return job;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
async fn duplicate_requests_are_ignored_while_active() {
    let app = tauri::test::mock_app();
    let dm = DownloadManager::new(1);
    let lib = temp_library("dup");
    // Concurrencia 1: el segundo queda en cola, y repetirlo no crea otro trabajo
    let first = dm.enqueue(app.handle(), JkClient::new(), &lib, vec![request(1), request(2)]);
    let again = dm.enqueue(app.handle(), JkClient::new(), &lib, vec![request(2)]);
    assert_eq!(first.len(), 2);
    assert!(again.is_empty(), "el episodio 2 ya estaba en cola");
    for id in first {
        dm.cancel(app.handle(), id);
    }
    let _ = std::fs::remove_dir_all(lib);
}

#[tokio::test]
async fn existing_complete_file_is_skipped() {
    let app = tauri::test::mock_app();
    let dm = DownloadManager::new(1);
    let lib = temp_library("skip");
    // Un MP4 válido mínimo generado con ffmpeg (si no hay ffmpeg, se omite la prueba)
    let out = library::episode_path(&lib, "Elfen Lied", 1, 3);
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let made = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", "color=black:s=64x64:d=1", "-y"])
        .arg(&out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !made {
        eprintln!("ffmpeg no disponible: prueba omitida");
        return;
    }
    let ids = dm.enqueue(app.handle(), JkClient::new(), &lib, vec![request(3)]);
    let job = wait_for(&dm, ids[0], 20, |s| matches!(s, Status::Skipped | Status::Failed { .. })).await;
    assert!(matches!(job.status, Status::Skipped), "estado: {:?}", job.status);
    let _ = std::fs::remove_dir_all(lib);
}

#[tokio::test]
#[ignore = "usa la red y descarga unos segundos de un episodio real"]
async fn live_download_progress_and_cancel() {
    let app = tauri::test::mock_app();
    let dm = DownloadManager::new(1);
    let lib = temp_library("live");
    let ids = dm.enqueue(app.handle(), JkClient::new(), &lib, vec![request(1)]);
    let id = ids[0];

    let job = wait_for(&dm, id, 120, |s| {
        matches!(s, Status::Downloading { percent: Some(p), .. } if *p > 0.5)
            || matches!(s, Status::Failed { .. })
    })
    .await;
    println!("estado tras arrancar: {:?}", job.status);
    assert!(matches!(job.status, Status::Downloading { .. }), "no llegó a descargar: {:?}", job.status);
    println!("destino: {}", job.output.display());
    assert!(job.output.ends_with("Elfen Lied/Season 01/Elfen Lied - S01E01.mp4"));

    dm.cancel(app.handle(), id);
    let job = wait_for(&dm, id, 20, |s| matches!(s, Status::Cancelled)).await;
    println!("estado tras cancelar: {:?}", job.status);
    assert!(matches!(job.status, Status::Cancelled));
    let _ = std::fs::remove_dir_all(lib);
}
