//! Cola de descargas con yt-dlp. Equivale a `download_episode` / `download_season` del
//! script: obtiene una URL fresca de cada episodio justo antes de descargarlo (los
//! `.m3u8` caducan), salta los que ya están completos y remuxea a MP4 sin recodificar.
//!
//! Además (v1.1): la cola se guarda en disco y se retoma al abrir la app, cada descarga
//! se puede pausar y reanudar (yt-dlp continúa los fragmentos ya bajados), los fallos
//! temporales se reintentan solos y al vaciarse la cola se avisa con una notificación.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Semaphore, watch};

use crate::jkanime::{JkClient, JkError, episode};
use crate::library::{self, Layout};
use crate::tools;

/// Evento que recibe la interfaz cada vez que cambia un trabajo.
pub const EVENT: &str = "download-updated";

/// Esperas entre reintentos automáticos de un fallo temporal (1, 5 y 15 minutos).
pub const RETRY_DELAYS: [u64; 3] = [60, 300, 900];

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Pausado por el usuario; los fragmentos descargados se conservan.
    Paused,
    /// Falló por un problema temporal y se reintentará solo en `retry_at` (segundos Unix).
    Waiting {
        attempt: u32,
        retry_at: u64,
        message: String,
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
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            Status::Done | Status::Skipped | Status::Failed { .. } | Status::Cancelled
        )
    }

    /// Hay un proceso o una espera en marcha para este trabajo.
    pub fn is_active(&self) -> bool {
        !self.is_finished() && !matches!(self, Status::Paused)
    }

    fn kind(&self) -> std::mem::Discriminant<Status> {
        std::mem::discriminant(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub anime: String,
    pub anime_url: String,
    pub is_movie: bool,
    pub season: u32,
    pub episode: u32,
    pub output: PathBuf,
    pub status: Status,
    /// Reintentos automáticos ya hechos.
    #[serde(default)]
    pub attempts: u32,
}

impl Job {
    /// "Elfen Lied T1 E3" (o "Akira (película)") para el registro.
    pub fn label(&self) -> String {
        if self.is_movie {
            format!("{} (película)", self.anime)
        } else {
            format!("{} T{} E{}", self.anime, self.season, self.episode)
        }
    }
}

#[derive(Debug, Clone)]
pub struct EpisodeRequest {
    pub anime: String,
    pub anime_url: String,
    pub is_movie: bool,
    pub season: u32,
    pub episode: u32,
}

/// Orden que recibe el trabajador de cada descarga.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Signal {
    Run,
    Pause,
    Cancel,
}

/// Cómo terminó un intento de descarga.
enum Outcome {
    Finished(Status),
    Paused,
    Cancelled,
    /// `transient`: tiene sentido reintentar más tarde (red, jkanime caído…).
    Failed {
        message: String,
        transient: bool,
    },
}

#[derive(Default)]
struct Batch {
    done: u32,
    skipped: u32,
    failed: u32,
}

struct Inner {
    jobs: Vec<Job>,
    signals: HashMap<u64, watch::Sender<Signal>>,
    next_id: u64,
    batch: Batch,
}

type ChangeHook = Arc<dyn Fn(&[Job]) + Send + Sync>;

#[derive(Clone)]
pub struct DownloadManager {
    inner: Arc<Mutex<Inner>>,
    permits: Arc<Semaphore>,
    client: JkClient,
    /// Archivo donde se guarda la cola (None en tests).
    state_file: Option<PathBuf>,
    on_change: Option<ChangeHook>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl DownloadManager {
    pub fn new(concurrent: u32, client: JkClient, state_file: Option<PathBuf>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                jobs: Vec::new(),
                signals: HashMap::new(),
                next_id: 1,
                batch: Batch::default(),
            })),
            permits: Arc::new(Semaphore::new(concurrent.max(1) as usize)),
            client,
            state_file,
            on_change: None,
        }
    }

    /// Función a la que se llama tras cada cambio (para el icono de la bandeja).
    pub fn with_hook(mut self, hook: impl Fn(&[Job]) + Send + Sync + 'static) -> Self {
        self.on_change = Some(Arc::new(hook));
        self
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.inner.lock().unwrap().jobs.clone()
    }

    pub fn has_active(&self) -> bool {
        self.inner
            .lock()
            .unwrap()
            .jobs
            .iter()
            .any(|j| j.status.is_active())
    }

    /// Carga la cola guardada y retoma lo que estaba pendiente. Lo pausado sigue pausado.
    pub fn restore<R: Runtime>(&self, app: &AppHandle<R>) {
        let Some(path) = &self.state_file else { return };
        let Ok(text) = std::fs::read_to_string(path) else {
            return;
        };
        let Ok(mut jobs) = serde_json::from_str::<Vec<Job>>(&text) else {
            return;
        };
        let mut resume = Vec::new();
        for job in &mut jobs {
            if job.status.is_active() {
                job.status = Status::Queued;
                resume.push(job.id);
            }
        }
        {
            let mut inner = self.inner.lock().unwrap();
            inner.next_id = jobs.iter().map(|j| j.id).max().unwrap_or(0) + 1;
            inner.jobs = jobs;
        }
        for id in resume {
            self.start(app, id);
        }
        self.changed();
    }

    /// Encola episodios. Los que ya están en cola, en curso o en pausa se ignoran.
    pub fn enqueue<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        library_dir: &Path,
        layout: Layout,
        requests: Vec<EpisodeRequest>,
    ) -> Vec<u64> {
        let mut ids = Vec::new();
        for req in requests {
            let output = library::output_path(
                library_dir,
                layout,
                &req.anime,
                req.is_movie,
                req.season,
                req.episode,
            );
            let job = {
                let mut inner = self.inner.lock().unwrap();
                if inner
                    .jobs
                    .iter()
                    .any(|j| j.output == output && !j.status.is_finished())
                {
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
                    attempts: 0,
                };
                inner.jobs.push(job.clone());
                job
            };
            ids.push(job.id);
            log::info!("{}: en cola → {}", job.label(), job.output.display());
            let _ = app.emit(EVENT, &job);
            self.start(app, job.id);
        }
        self.changed();
        ids
    }

    /// Lanza el trabajador de una descarga (que espera su turno en la cola).
    fn start<R: Runtime>(&self, app: &AppHandle<R>, id: u64) {
        let (tx, rx) = watch::channel(Signal::Run);
        self.inner.lock().unwrap().signals.insert(id, tx);
        let this = self.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move { this.worker(&app, id, rx).await });
    }

    async fn worker<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        id: u64,
        mut signal: watch::Receiver<Signal>,
    ) {
        loop {
            // Esperar turno; se puede pausar o cancelar mientras tanto
            let permit = tokio::select! {
                p = self.permits.clone().acquire_owned() => p.ok(),
                _ = signal.changed() => None,
            };
            if let Some(stop) = stop_status(*signal.borrow()) {
                self.set_status(app, id, stop);
                return;
            }
            let Some(_permit) = permit else { return };
            let Some(job) = self.get(id) else { return };

            let label = job.label();
            match self.attempt(app, &job, &mut signal).await {
                Outcome::Finished(status) => {
                    let skipped = matches!(status, Status::Skipped);
                    log::info!(
                        "{label}: {}",
                        if skipped {
                            "ya estaba en la biblioteca"
                        } else {
                            "descargado"
                        }
                    );
                    self.set_status(app, id, status);
                    self.finished(app, if skipped { Final::Skipped } else { Final::Done });
                    return;
                }
                Outcome::Paused => return self.set_status(app, id, Status::Paused),
                Outcome::Cancelled => return self.set_status(app, id, Status::Cancelled),
                Outcome::Failed { message, transient } => {
                    let attempts = self.bump_attempts(id);
                    let delay = RETRY_DELAYS.get(attempts as usize - 1).copied();
                    let Some(delay) = delay.filter(|_| transient) else {
                        log::error!("{label}: falló (intento {attempts}): {message}");
                        self.set_status(app, id, Status::Failed { message });
                        self.finished(app, Final::Failed);
                        return;
                    };
                    log::warn!(
                        "{label}: fallo temporal (intento {attempts}), reintento en {delay} s: {message}"
                    );
                    drop(_permit);
                    let retry_at = now_secs() + delay;
                    self.set_status(
                        app,
                        id,
                        Status::Waiting {
                            attempt: attempts,
                            retry_at,
                            message,
                        },
                    );
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
                        _ = signal.changed() => {}
                    }
                    if let Some(stop) = stop_status(*signal.borrow()) {
                        self.set_status(app, id, stop);
                        return;
                    }
                    self.set_status(app, id, Status::Queued);
                }
            }
        }
    }

    fn get(&self, id: u64) -> Option<Job> {
        self.inner
            .lock()
            .unwrap()
            .jobs
            .iter()
            .find(|j| j.id == id)
            .cloned()
    }

    fn bump_attempts(&self, id: u64) -> u32 {
        let mut inner = self.inner.lock().unwrap();
        inner
            .jobs
            .iter_mut()
            .find(|j| j.id == id)
            .map(|j| {
                j.attempts += 1;
                j.attempts
            })
            .unwrap_or(1)
    }

    fn send(&self, id: u64, signal: Signal) -> bool {
        let tx = self.inner.lock().unwrap().signals.get(&id).cloned();
        tx.map(|tx| tx.send(signal).is_ok()).unwrap_or(false)
    }

    pub fn pause<R: Runtime>(&self, app: &AppHandle<R>, id: u64) {
        if self.get(id).is_some_and(|j| j.status.is_active()) && !self.send(id, Signal::Pause) {
            self.set_status(app, id, Status::Paused);
        }
    }

    /// Reanuda una pausada o vuelve a intentar una fallida/cancelada.
    pub fn resume<R: Runtime>(&self, app: &AppHandle<R>, id: u64) {
        let Some(job) = self.get(id) else { return };
        if job.status.is_active() || matches!(job.status, Status::Done | Status::Skipped) {
            return;
        }
        {
            let mut inner = self.inner.lock().unwrap();
            if let Some(j) = inner.jobs.iter_mut().find(|j| j.id == id) {
                j.attempts = 0;
                j.status = Status::Queued;
                let _ = app.emit(EVENT, &*j);
            }
        }
        self.start(app, id);
        self.changed();
    }

    pub fn cancel<R: Runtime>(&self, app: &AppHandle<R>, id: u64) {
        let Some(job) = self.get(id) else { return };
        if job.status.is_finished() {
            return;
        }
        // Una descarga pausada no tiene trabajador: se marca directamente
        if matches!(job.status, Status::Paused) || !self.send(id, Signal::Cancel) {
            self.set_status(app, id, Status::Cancelled);
        }
    }

    pub fn pause_all<R: Runtime>(&self, app: &AppHandle<R>) {
        for j in self.jobs().into_iter().filter(|j| j.status.is_active()) {
            self.pause(app, j.id);
        }
    }

    pub fn resume_all<R: Runtime>(&self, app: &AppHandle<R>) {
        for j in self
            .jobs()
            .into_iter()
            .filter(|j| matches!(j.status, Status::Paused))
        {
            self.resume(app, j.id);
        }
    }

    /// Quita de la lista los trabajos terminados.
    pub fn clear_finished(&self) {
        {
            let mut inner = self.inner.lock().unwrap();
            let finished: Vec<u64> = inner
                .jobs
                .iter()
                .filter(|j| j.status.is_finished())
                .map(|j| j.id)
                .collect();
            inner.jobs.retain(|j| !j.status.is_finished());
            for id in finished {
                inner.signals.remove(&id);
            }
        }
        self.changed();
    }

    fn set_status<R: Runtime>(&self, app: &AppHandle<R>, id: u64, status: Status) {
        let (job, kind_changed) = {
            let mut inner = self.inner.lock().unwrap();
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            // Un trabajo cancelado solo sale de ahí con `resume`
            if matches!(job.status, Status::Cancelled) {
                return;
            }
            let kind_changed = job.status.kind() != status.kind();
            job.status = status;
            (job.clone(), kind_changed)
        };
        let _ = app.emit(EVENT, &job);
        // El progreso cambia muchas veces por segundo: solo se guarda al cambiar de estado
        if kind_changed {
            self.changed();
        } else if let Some(hook) = &self.on_change {
            hook(&self.jobs());
        }
    }

    /// Guarda la cola y avisa al icono de la bandeja.
    fn changed(&self) {
        let jobs = self.jobs();
        if let Some(path) = &self.state_file {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Ok(text) = serde_json::to_string_pretty(&jobs) {
                let tmp = path.with_extension("tmp");
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(&tmp, path);
                }
            }
        }
        if let Some(hook) = &self.on_change {
            hook(&jobs);
        }
    }

    /// Cuenta el resultado y, si ya no queda nada en marcha, notifica el resumen.
    fn finished<R: Runtime>(&self, app: &AppHandle<R>, result: Final) {
        let summary = {
            let mut inner = self.inner.lock().unwrap();
            match result {
                Final::Done => inner.batch.done += 1,
                Final::Skipped => inner.batch.skipped += 1,
                Final::Failed => inner.batch.failed += 1,
            }
            if inner.jobs.iter().any(|j| j.status.is_active()) {
                None
            } else {
                Some(std::mem::take(&mut inner.batch))
            }
        };
        if let Some(b) = summary {
            let mut parts = Vec::new();
            if b.done > 0 {
                parts.push(format!("{} descargado{}", b.done, plural(b.done)));
            }
            if b.skipped > 0 {
                parts.push(format!(
                    "{} ya estaba{} en la biblioteca",
                    b.skipped,
                    plural(b.skipped)
                ));
            }
            if b.failed > 0 {
                parts.push(format!("{} fallido{}", b.failed, plural(b.failed)));
            }
            notify(
                app,
                "Descargas terminadas",
                &format!("{}.", parts.join(", ")),
            );
        }
    }

    /// Un intento completo: comprobar disco, buscar la URL y ejecutar yt-dlp.
    async fn attempt<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        job: &Job,
        signal: &mut watch::Receiver<Signal>,
    ) -> Outcome {
        if is_complete(&job.output).await {
            return Outcome::Finished(Status::Skipped);
        }

        self.set_status(app, job.id, Status::Resolving);
        let source = tokio::select! {
            r = episode::fetch_video_source(&self.client, &job.anime_url, job.episode, job.is_movie) => r,
            _ = signal.changed() => return stop_outcome(*signal.borrow()),
        };
        let source = match source {
            Ok(s) => {
                log::debug!("{}: fuente {:?}", job.label(), s.kind);
                s
            }
            Err(e) => {
                let transient = matches!(e, JkError::Unavailable(_));
                return Outcome::Failed {
                    message: e.to_string(),
                    transient,
                };
            }
        };

        if let Some(dir) = job.output.parent()
            && let Err(e) = tokio::fs::create_dir_all(dir).await
        {
            return Outcome::Failed {
                message: format!("no se pudo crear {}: {e}", dir.display()),
                transient: false,
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
                log::error!("no se pudo ejecutar {}: {e}", tools::ytdlp().display());
                return Outcome::Failed {
                    message: format!("no se pudo ejecutar yt-dlp ({e}). ¿Está instalado?"),
                    transient: false,
                };
            }
        };

        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        let mut lines = BufReader::new(stdout).lines();
        // Última línea con ERROR (para la interfaz) y las últimas líneas (para el registro)
        let errors = tokio::spawn(async move {
            let mut last = String::new();
            let mut tail = std::collections::VecDeque::new();
            let mut l = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = l.next_line().await {
                if line.contains("ERROR") {
                    last = line.clone();
                }
                if tail.len() == 15 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
            (last, Vec::from(tail).join("\n"))
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
                _ = signal.changed() => {
                    // Pausar o cancelar: se corta yt-dlp; los fragmentos quedan para continuar
                    let _ = child.kill().await;
                    return stop_outcome(*signal.borrow());
                }
            }
        }

        let exit = child.wait().await;
        let ok = exit.as_ref().map(|s| s.success()).unwrap_or(false);
        let (err, tail) = errors.await.unwrap_or_default();
        if !ok {
            log::warn!(
                "{}: yt-dlp terminó con {:?}. Últimas líneas:\n{tail}",
                job.label(),
                exit.map(|s| s.code())
            );
        }
        if ok && tokio::fs::try_exists(&job.output).await.unwrap_or(false) {
            // Restos de un intento anterior cortado de golpe (cierre de la app, apagón):
            // con el MP4 ya terminado sobran y harían que el episodio pareciera incompleto
            remove_partials(&job.output);
            Outcome::Finished(Status::Done)
        } else {
            let message = if err.is_empty() {
                "yt-dlp terminó sin generar el archivo".to_string()
            } else {
                err
            };
            // Casi todos los fallos de yt-dlp son de red o de un enlace caducado: reintentar
            // con una URL nueva tiene sentido. Lo que no, es un enlace que no sabe leer.
            let transient = !message.contains("Unsupported URL");
            Outcome::Failed { message, transient }
        }
    }
}

/// Resultado final de un episodio, para el resumen de la notificación.
enum Final {
    Done,
    Skipped,
    Failed,
}

fn plural(n: u32) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Estado final si el trabajador recibió una orden de parar.
fn stop_status(signal: Signal) -> Option<Status> {
    match signal {
        Signal::Run => None,
        Signal::Pause => Some(Status::Paused),
        Signal::Cancel => Some(Status::Cancelled),
    }
}

fn stop_outcome(signal: Signal) -> Outcome {
    match signal {
        Signal::Cancel => Outcome::Cancelled,
        _ => Outcome::Paused,
    }
}

/// Notificación de escritorio (si el plugin está cargado; en tests no lo está).
pub fn notify<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if app
        .try_state::<tauri_plugin_notification::Notification<R>>()
        .is_some()
    {
        let _ = app.notification().builder().title(title).body(body).show();
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

/// Archivos temporales de yt-dlp de un episodio (`<nombre>.mp4.part`, `…part-Frag12.part`,
/// `.ytdl`).
fn partials(output: &Path) -> Vec<PathBuf> {
    let (Some(dir), Some(stem)) = (output.parent(), output.file_stem().and_then(|s| s.to_str()))
    else {
        return Vec::new();
    };
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    let n = e.file_name().to_string_lossy().to_string();
                    n.starts_with(&format!("{stem}."))
                        && (n.contains(".part") || n.ends_with(".ytdl"))
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default()
}

fn has_partials(output: &Path) -> bool {
    !partials(output).is_empty()
}

fn remove_partials(output: &Path) {
    for p in partials(output) {
        match std::fs::remove_file(&p) {
            Ok(()) => log::info!("borrado resto de descarga: {}", p.display()),
            Err(e) => log::warn!("no se pudo borrar {}: {e}", p.display()),
        }
    }
}

/// Episodios de un anime que ya están completos en la biblioteca.
pub async fn episodes_on_disk(
    library_dir: &Path,
    layout: Layout,
    anime: &str,
    is_movie: bool,
    season: u32,
    numbers: &[u32],
) -> Vec<u32> {
    let mut present = Vec::new();
    for &n in numbers {
        let p = library::output_path(library_dir, layout, anime, is_movie, season, n);
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
    fn leftover_fragments_are_found_and_removed() {
        let dir = std::env::temp_dir().join(format!("the-wired-partials-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("Higurashi - S01E20.mp4");
        let keep = dir.join("Higurashi - S01E21.mp4.part-Frag3.part");
        std::fs::write(&out, b"mp4").unwrap();
        std::fs::write(dir.join("Higurashi - S01E20.mp4.part-Frag295.part"), b"x").unwrap();
        std::fs::write(dir.join("Higurashi - S01E20.mp4.ytdl"), b"x").unwrap();
        std::fs::write(&keep, b"x").unwrap();

        assert!(has_partials(&out));
        remove_partials(&out);
        assert!(!has_partials(&out));
        assert!(out.exists(), "el vídeo no se toca");
        assert!(keep.exists(), "los restos de otro episodio no se tocan");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_progress_lines_are_ignored() {
        assert!(parse_progress("[download] Destination: x.mp4").is_none());
        assert!(parse_progress("").is_none());
    }
}
