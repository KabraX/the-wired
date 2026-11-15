//! Pruebas del gestor de descargas con una app de Tauri simulada.
//!
//!   cargo test --test downloads                         (sin red)
//!   cargo test --test downloads -- --ignored --nocapture (descarga real unos segundos)

// Usa `tauri::test::mock_app`: en Windows los binarios de test no llevan el manifiesto
// de la app y Windows no los arranca (STATUS_ENTRYPOINT_NOT_FOUND). La lógica es la
// misma en ambos sistemas, así que estos tests se ejecutan solo fuera de Windows.
#![cfg(not(windows))]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use the_wired_lib::download::{DownloadManager, EpisodeRequest, Job, Status, YoutubeRequest};
use the_wired_lib::jkanime::JkClient;
use the_wired_lib::library::{self, Layout};
use the_wired_lib::youtube::{AudioCodec, YtFormat, YtItem};

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
    let dm = DownloadManager::new(1, JkClient::new(), None);
    let lib = temp_library("dup");
    // Concurrencia 1: el segundo queda en cola, y repetirlo no crea otro trabajo
    let first = dm.enqueue(
        app.handle(),
        &lib,
        Layout::Jellyfin,
        vec![request(1), request(2)],
    );
    let again = dm.enqueue(app.handle(), &lib, Layout::Jellyfin, vec![request(2)]);
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
    let dm = DownloadManager::new(1, JkClient::new(), None);
    let lib = temp_library("skip");
    // Un MP4 válido mínimo generado con ffmpeg (si no hay ffmpeg, se omite la prueba)
    let out = library::episode_path(&lib, "Elfen Lied", 1, 3);
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let made = std::process::Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=black:s=64x64:d=1",
            "-y",
        ])
        .arg(&out)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !made {
        eprintln!("ffmpeg no disponible: prueba omitida");
        return;
    }
    let ids = dm.enqueue(app.handle(), &lib, Layout::Jellyfin, vec![request(3)]);
    let job = wait_for(&dm, ids[0], 20, |s| {
        matches!(s, Status::Skipped | Status::Failed { .. })
    })
    .await;
    assert!(
        matches!(job.status, Status::Skipped),
        "estado: {:?}",
        job.status
    );
    let _ = std::fs::remove_dir_all(lib);
}

#[tokio::test]
#[ignore = "usa la red y descarga unos segundos de un episodio real"]
async fn live_download_progress_and_cancel() {
    let app = tauri::test::mock_app();
    let dm = DownloadManager::new(1, JkClient::new(), None);
    let lib = temp_library("live");
    let ids = dm.enqueue(app.handle(), &lib, Layout::Jellyfin, vec![request(1)]);
    let id = ids[0];

    let job = wait_for(&dm, id, 120, |s| {
        matches!(s, Status::Downloading { percent: Some(p), .. } if *p > 0.5)
            || matches!(s, Status::Failed { .. })
    })
    .await;
    println!("estado tras arrancar: {:?}", job.status);
    assert!(
        matches!(job.status, Status::Downloading { .. }),
        "no llegó a descargar: {:?}",
        job.status
    );
    println!("destino: {}", job.output.display());
    assert!(
        job.output
            .ends_with("Elfen Lied/Season 01/Elfen Lied - S01E01.mp4")
    );

    // Pausar y reanudar: yt-dlp debe continuar con los fragmentos ya bajados
    dm.pause(app.handle(), id);
    let job = wait_for(&dm, id, 20, |s| matches!(s, Status::Paused)).await;
    println!("estado tras pausar: {:?}", job.status);
    assert!(matches!(job.status, Status::Paused));
    dm.resume(app.handle(), id);
    let job = wait_for(
        &dm,
        id,
        120,
        |s| matches!(s, Status::Downloading { percent: Some(p), .. } if *p > 0.0),
    )
    .await;
    println!("estado tras reanudar: {:?}", job.status);
    assert!(matches!(job.status, Status::Downloading { .. }));

    dm.cancel(app.handle(), id);
    let job = wait_for(&dm, id, 20, |s| matches!(s, Status::Cancelled)).await;
    println!("estado tras cancelar: {:?}", job.status);
    assert!(matches!(job.status, Status::Cancelled));
    let _ = std::fs::remove_dir_all(lib);
}

fn unreachable_request(episode: u32) -> EpisodeRequest {
    // Puerto 9 en local: conexión rechazada, igual que con jkanime caído
    EpisodeRequest {
        anime_url: "http://127.0.0.1:9/elfen-lied".into(),
        ..request(episode)
    }
}

#[tokio::test]
async fn temporary_failure_waits_to_retry() {
    let app = tauri::test::mock_app();
    let dm = DownloadManager::new(1, JkClient::new(), None);
    let lib = temp_library("retry");
    let id = dm.enqueue(
        app.handle(),
        &lib,
        Layout::Jellyfin,
        vec![unreachable_request(1)],
    )[0];
    let job = wait_for(&dm, id, 60, |s| {
        matches!(s, Status::Waiting { .. } | Status::Failed { .. })
    })
    .await;
    match &job.status {
        Status::Waiting {
            attempt, retry_at, ..
        } => {
            assert_eq!(*attempt, 1);
            assert!(*retry_at > 0);
        }
        other => panic!("debería esperar para reintentar, está en {other:?}"),
    }
    // Pausar durante la espera
    dm.pause(app.handle(), id);
    let job = wait_for(&dm, id, 10, |s| matches!(s, Status::Paused)).await;
    assert!(matches!(job.status, Status::Paused), "{:?}", job.status);
    // Cancelar una pausada es inmediato
    dm.cancel(app.handle(), id);
    assert!(matches!(dm.jobs()[0].status, Status::Cancelled));
    let _ = std::fs::remove_dir_all(lib);
}

#[tokio::test]
async fn queue_is_saved_and_restored() {
    let app = tauri::test::mock_app();
    let lib = temp_library("persist");
    let state = lib.join("downloads.json");
    {
        let dm = DownloadManager::new(1, JkClient::new(), Some(state.clone()));
        let ids = dm.enqueue(
            app.handle(),
            &lib,
            Layout::Jellyfin,
            vec![unreachable_request(1), unreachable_request(2)],
        );
        // El segundo queda pausado antes de empezar
        dm.pause(app.handle(), ids[1]);
        wait_for(&dm, ids[1], 10, |s| matches!(s, Status::Paused)).await;
        dm.pause(app.handle(), ids[0]);
        wait_for(&dm, ids[0], 60, |s| matches!(s, Status::Paused)).await;
        dm.resume(app.handle(), ids[0]);
    }
    assert!(state.exists(), "la cola debe guardarse en disco");

    // "Reabrir la app": un gestor nuevo con el mismo archivo
    let dm = DownloadManager::new(1, JkClient::new(), Some(state.clone()));
    dm.restore(app.handle());
    let jobs = dm.jobs();
    assert_eq!(jobs.len(), 2);
    assert!(
        !matches!(jobs[0].status, Status::Paused),
        "el que estaba activo se retoma: {:?}",
        jobs[0].status
    );
    assert!(
        matches!(jobs[1].status, Status::Paused),
        "el pausado sigue pausado: {:?}",
        jobs[1].status
    );
    // Los ids nuevos no chocan con los restaurados
    let new = dm.enqueue(
        app.handle(),
        &lib,
        Layout::Jellyfin,
        vec![unreachable_request(3)],
    );
    assert!(new[0] > jobs.iter().map(|j| j.id).max().unwrap());
    for j in dm.jobs() {
        dm.cancel(app.handle(), j.id);
    }
    let _ = std::fs::remove_dir_all(lib);
}

// ---------------------------------------------------------------- YouTube

fn yt_item(url: &str, title: &str, index: u32) -> YtItem {
    YtItem {
        url: url.to_string(),
        title: title.to_string(),
        index: Some(index),
    }
}

fn yt_request(index: u32, format: YtFormat) -> YoutubeRequest {
    YoutubeRequest {
        folder: Some("My List".into()),
        url: "https://www.youtube.com/playlist?list=PL1".into(),
        item: yt_item(
            &format!("https://www.youtube.com/watch?v=a{index}"),
            &format!("Video {index}"),
            index,
        ),
        format,
    }
}

#[test]
fn old_queue_entries_load_without_youtube_fields() {
    let old = r#"[{"id":1,"anime":"Elfen Lied","anime_url":"https://jkanime.net/elfen-lied","is_movie":false,"season":1,"episode":1,"output":"/tmp/Elfen Lied/Season 01/Elfen Lied - S01E01.mp4","status":{"state":"done"},"attempts":0}]"#;
    let jobs: Vec<Job> = serde_json::from_str(old).unwrap();
    assert_eq!(jobs.len(), 1);
    assert!(jobs[0].yt_format.is_none());
    assert!(jobs[0].video_url.is_none());
    assert!(jobs[0].video_title.is_none());
}

#[test]
fn youtube_format_serializes_for_the_frontend() {
    let video = serde_json::to_value(YtFormat::Video {
        max_height: Some(1080),
    })
    .unwrap();
    assert_eq!(video["kind"], "video");
    assert_eq!(video["max_height"], 1080);
    let audio = serde_json::to_value(YtFormat::Audio {
        codec: AudioCodec::M4a,
    })
    .unwrap();
    assert_eq!(audio["kind"], "audio");
    assert_eq!(audio["codec"], "m4a");
}

#[tokio::test]
async fn youtube_jobs_have_their_own_paths_and_are_deduplicated() {
    let app = tauri::test::mock_app();
    let dm = DownloadManager::new(1, JkClient::new(), None);
    let lib = temp_library("yt");
    let ids = dm.enqueue_youtube(
        app.handle(),
        &lib,
        vec![
            yt_request(
                1,
                YtFormat::Video {
                    max_height: Some(720),
                },
            ),
            yt_request(
                2,
                YtFormat::Audio {
                    codec: AudioCodec::Mp3,
                },
            ),
        ],
    );
    assert_eq!(ids.len(), 2);
    let jobs = dm.jobs();
    assert_eq!(jobs[0].anime, "My List");
    assert_eq!(jobs[0].video_title.as_deref(), Some("Video 1"));
    assert_eq!(jobs[0].episode, 1);
    assert!(
        jobs[0].output.ends_with("My List/01 - Video 1.mp4"),
        "{}",
        jobs[0].output.display()
    );
    assert!(
        jobs[1].output.ends_with("My List/02 - Video 2.mp3"),
        "{}",
        jobs[1].output.display()
    );
    // Repetir un vídeo ya encolado no crea otro trabajo
    let again = dm.enqueue_youtube(
        app.handle(),
        &lib,
        vec![yt_request(
            1,
            YtFormat::Video {
                max_height: Some(720),
            },
        )],
    );
    assert!(again.is_empty());
    for j in dm.jobs() {
        dm.cancel(app.handle(), j.id);
    }
    let _ = std::fs::remove_dir_all(lib);
}
