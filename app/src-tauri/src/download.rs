//! Cola de descargas con yt-dlp. Equivale a `download_episode` / `download_season` del
//! script: obtiene una URL fresca de cada episodio justo antes de descargarlo (los
//! `.m3u8` caducan), salta los que ya están completos y remuxea a MP4 sin recodificar.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Semaphore, watch};

use crate::jkanime::{JkClient, episode};
use crate::library;
use crate::tools;

/// Evento que recibe la interfaz cada vez que cambia un trabajo.
pub const EVENT: &str = "download-updated";

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum Status {
    Queued,
    /// Buscando la URL del vídeo en jkanime.
    Resolving,
    Downloading {
        /// 0.0 – 100.0, si se conoce.
        percent: Option<f64>,
        /// Bytes por segundo.
        speed: Option<f64>,
        /// Segundos restantes.
        eta: Option<u64>,
    },
    Done,
    /// Ya existía completo en la biblioteca.
    Skipped,
    Failed {
        message: String,
    },
    Cancelled,
}

impl Status {
    fn is_finished(&self) -> bool {
        matches!(
            self,
            Status::Done | Status::Skipped | Status::Failed { .. } | Status::Cancelled
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: u64,
    pub anime: String,
    pub anime_url: String,
    pub is_movie: bool,
    pub season: u32,
    pub episode: u32,
    pub output: PathBuf,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub struct EpisodeRequest {
    pub anime: String,
    pub anime_url: String,
    pub is_movie: bool,
    pub season: u32,
    pub episode: u32,
}

struct Inner {
    jobs: Vec<Job>,
    cancels: HashMap<u64, watch::Sender<bool>>,
    next_id: u64,
}

#[derive(Clone)]
pub struct DownloadManager {
    inner: Arc<Mutex<Inner>>,
    permits: Arc<Semaphore>,
}

impl DownloadManager {
    pub fn new(concurrent: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                jobs: Vec::new(),
                cancels: HashMap::new(),
                next_id: 1,
            })),
            permits: Arc::new(Semaphore::new(concurrent.max(1) as usize)),
        }
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.inner.lock().unwrap().jobs.clone()
    }

    /// Encola episodios. Los que ya están en cola o descargándose se ignoran.
    pub fn enqueue<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        client: JkClient,
        library_dir: &Path,
        requests: Vec<EpisodeRequest>,
    ) -> Vec<u64> {
        let mut ids = Vec::new();
        for req in requests {
            let output = library::episode_path(library_dir, &req.anime, req.season, req.episode);
            let (tx, rx) = watch::channel(false);
            let job = {
                let mut inner = self.inner.lock().unwrap();
                let busy = inner
                    .jobs
                    .iter()
                    .any(|j| j.output == output && !j.status.is_finished());
                if busy {
                    continue;
                }
                let id = inner.next_id;
                inner.next_id += 1;
                let job = Job {
                    id,
                    anime: req.anime.clone(),
                    anime_url: req.anime_url.clone(),
                    is_movie: req.is_movie,
                    season: req.season,
                    episode: req.episode,
                    output,
                    status: Status::Queued,
                };
                inner.jobs.push(job.clone());
                inner.cancels.insert(id, tx);
                job
            };
            ids.push(job.id);
            let _ = app.emit(EVENT, &job);

            let this = self.clone();
            let app = app.clone();
            let client = client.clone();
            tauri::async_runtime::spawn(async move {
                let _permit = this.permits.clone().acquire_owned().await;
                if *rx.borrow() {
                    return; // cancelado mientras esperaba turno
                }
                let status = this.run(&app, &client, &job, rx).await;
                this.set_status(&app, job.id, status);
            });
        }
        ids
    }

    pub fn cancel<R: Runtime>(&self, app: &AppHandle<R>, id: u64) {
        let tx = self.inner.lock().unwrap().cancels.get(&id).cloned();
        if let Some(tx) = tx {
            let _ = tx.send(true);
        }
        let queued = self
            .inner
            .lock()
            .unwrap()
            .jobs
            .iter()
            .any(|j| j.id == id && matches!(j.status, Status::Queued));
        if queued {
            self.set_status(app, id, Status::Cancelled);
        }
    }

    /// Quita de la lista los trabajos terminados.
    pub fn clear_finished(&self) {
        let mut inner = self.inner.lock().unwrap();
        let finished: Vec<u64> = inner
            .jobs
            .iter()
            .filter(|j| j.status.is_finished())
            .map(|j| j.id)
            .collect();
        inner.jobs.retain(|j| !j.status.is_finished());
        for id in finished {
            inner.cancels.remove(&id);
        }
    }

    fn set_status<R: Runtime>(&self, app: &AppHandle<R>, id: u64, status: Status) {
        let job = {
            let mut inner = self.inner.lock().unwrap();
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            // Un trabajo cancelado no vuelve a otro estado
            if matches!(job.status, Status::Cancelled) {
                return;
            }
            job.status = status;
            job.clone()
        };
        let _ = app.emit(EVENT, &job);
    }

    async fn run<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        client: &JkClient,
        job: &Job,
        mut cancel: watch::Receiver<bool>,
    ) -> Status {
        if is_complete(&job.output).await {
            return Status::Skipped;
        }

        self.set_status(app, job.id, Status::Resolving);
        let source = tokio::select! {
            r = episode::fetch_video_source(client, &job.anime_url, job.episode, job.is_movie) => r,
            _ = cancel.changed() => return Status::Cancelled,
        };
        let source = match source {
            Ok(s) => s,
            Err(e) => {
                return Status::Failed {
                    message: e.to_string(),
                };
            }
        };

        if let Some(dir) = job.output.parent()
            && let Err(e) = tokio::fs::create_dir_all(dir).await
        {
            return Status::Failed {
                message: format!("no se pudo crear {}: {e}", dir.display()),
            };
        }

        let template = job.output.with_extension("%(ext)s");
        let mut cmd = tools::command(tools::ytdlp());
        if let Some(dir) = tools::managed_ffmpeg_dir() {
            cmd.arg("--ffmpeg-location").arg(dir);
        }
        cmd.args([
            "--newline",
            "--no-colors",
            "--concurrent-fragments",
            "4",
            "--merge-output-format",
            "mp4",
            "--progress-template",
            PROGRESS_TEMPLATE,
            "-o",
        ])
        .arg(&template)
        .arg(&source.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Status::Failed {
                    message: format!("no se pudo ejecutar yt-dlp ({e}). ¿Está instalado?"),
                };
            }
        };

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        let mut lines = BufReader::new(stdout).lines();
        let errors = tokio::spawn(async move {
            let mut last = String::new();
            let mut l = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = l.next_line().await {
                if line.contains("ERROR") {
                    last = line;
                }
            }
            last
        });

        loop {
            tokio::select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => {
                        if let Some(p) = parse_progress(&line) {
                            self.set_status(app, job.id, p);
                        }
                    }
                    _ => break,
                },
                _ = cancel.changed() => {
                    let _ = child.kill().await;
                    return Status::Cancelled;
                }
            }
        }

        let ok = child.wait().await.map(|s| s.success()).unwrap_or(false);
        let err = errors.await.unwrap_or_default();
        if ok && tokio::fs::try_exists(&job.output).await.unwrap_or(false) {
            Status::Done
        } else if !err.is_empty() {
            Status::Failed { message: err }
        } else {
            Status::Failed {
                message: "yt-dlp terminó sin generar el archivo".into(),
            }
        }
    }
}

const PROGRESS_TEMPLATE: &str = "download:TW|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|\
%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|\
%(progress.fragment_index)s|%(progress.fragment_count)s";

/// Interpreta una línea de progreso de yt-dlp generada con `PROGRESS_TEMPLATE`.
pub fn parse_progress(line: &str) -> Option<Status> {
    let rest = line.trim().strip_prefix("TW|")?;
    let f: Vec<Option<f64>> = rest
        .split('|')
        .map(|v| v.trim().parse::<f64>().ok())
        .collect();
    if f.len() < 7 {
        return None;
    }
    let (done, total, estimate, speed, eta, frag, frags) =
        (f[0], f[1], f[2], f[3], f[4], f[5], f[6]);
    // En HLS el tamaño total es una estimación muy variable al principio (64 MB y luego
    // 612 MB en la misma descarga): los fragmentos dan un porcentaje más estable.
    let percent = match (frag, frags) {
        (Some(i), Some(n)) if n > 0.0 => Some(i / n * 100.0),
        _ => match (done, total.or(estimate)) {
            (Some(d), Some(t)) if t > 0.0 => Some(d / t * 100.0),
            _ => None,
        },
    }
    .map(|p| p.clamp(0.0, 100.0));
    Some(Status::Downloading {
        percent,
        speed,
        eta: eta.map(|e| e.max(0.0) as u64),
    })
}

/// Un episodio está completo si el MP4 existe, no quedan restos `.part` y, si ffprobe
/// está disponible, el archivo es un vídeo válido (mismo criterio que el script).
pub async fn is_complete(output: &Path) -> bool {
    if !tokio::fs::try_exists(output).await.unwrap_or(false) {
        return false;
    }
    if has_partials(output) {
        return false;
    }
    let Some(ffprobe) = tools::ffprobe() else {
        return true; // sin ffprobe, damos por bueno el archivo existente
    };
    match tools::command(ffprobe)
        .args(["-v", "error"])
        .arg(output)
        .output()
        .await
    {
        Ok(out) => out.status.success(),
        Err(_) => true,
    }
}

fn has_partials(output: &Path) -> bool {
    let (Some(dir), Some(stem)) = (output.parent(), output.file_stem().and_then(|s| s.to_str()))
    else {
        return false;
    };
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.starts_with(&format!("{stem}.")) && (n.contains(".part") || n.ends_with(".ytdl"))
            })
        })
        .unwrap_or(false)
}

/// Episodios de un anime que ya están completos en la biblioteca.
pub async fn episodes_on_disk(
    library_dir: &Path,
    anime: &str,
    season: u32,
    numbers: &[u32],
) -> Vec<u32> {
    let mut present = Vec::new();
    for &n in numbers {
        let p = library::episode_path(library_dir, anime, season, n);
        if tokio::fs::try_exists(&p).await.unwrap_or(false) && !has_partials(&p) {
            present.push(n);
        }
    }
    present
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pct(line: &str) -> (Option<f64>, Option<f64>, Option<u64>) {
        match parse_progress(line) {
            Some(Status::Downloading {
                percent,
                speed,
                eta,
            }) => (percent, speed, eta),
            other => panic!("no es progreso: {other:?}"),
        }
    }

    #[test]
    fn progress_lines_from_real_ytdlp_hls() {
        // Líneas reales de yt-dlp 2026.10.21 descargando un .m3u8 de jkanime
        let (p, s, e) = pct("TW|1024|NA|64736672.0|362.55415878467|NA|0|364");
        assert_eq!(p, Some(0.0));
        assert!(s.unwrap() > 300.0);
        assert_eq!(e, None);
        let (p, _, e) = pct("TW|5238514|NA|612627834.0|582273.5663224835|1071.8373871652145|4|364");
        assert!((p.unwrap() - 4.0 / 364.0 * 100.0).abs() < 1e-9);
        assert_eq!(e, Some(1071));
    }

    #[test]
    fn progress_without_fragments_uses_bytes() {
        let (p, _, _) = pct("TW|50|100|NA|10|5|NA|NA");
        assert_eq!(p, Some(50.0));
    }

    #[test]
    fn non_progress_lines_are_ignored() {
        assert!(parse_progress("[download] Destination: x.mp4").is_none());
        assert!(parse_progress("").is_none());
    }
}
