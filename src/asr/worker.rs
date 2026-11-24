//! The transcription worker: one thread that downloads the model and turns saved recordings
//! into transcripts, one job at a time.
//!
//! The UI talks to it through an `AsrHandle` (commands in, events out, like the audio worker).
//! Jobs sent while one runs wait in a queue and run in order, each folder once. The recognizer
//! is loaded for the first job and kept for the next ones; after `idle_unload` without a job
//! it is freed (the model takes over 1 GB of memory). A job that is cancelled, fails or is cut
//! short by closing the app leaves no transcript files, and the recordings are never changed.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::asr::download;
use crate::asr::engine::{Language, OrtRecognizer, Recognizer};
use crate::asr::glossary::Glossary;
use crate::asr::live::LiveTrack;
use crate::asr::model::{self, ModelStatus, Precision};
use crate::asr::segment::segment_file;
use crate::asr::transcript::{self, Line, ModelInfo, Track, Transcript};
use crate::asr::wavread::WavReader;

const MODEL_NAME: &str = "OpenThai2.0-ASR-Streaming-ONNX";
/// Sample rate the segments are numbered in.
const RATE: u64 = 16_000;
/// How often the download progress is passed on.
const DOWNLOAD_EVENT_EVERY: Duration = Duration::from_millis(100);

#[derive(Clone, Debug)]
pub enum Command {
    /// Report whether the model is installed.
    CheckModel {
        precision: Precision,
    },
    DownloadModel {
        precision: Precision,
    },
    /// Stops a running download; the bytes saved stay for the next one.
    CancelDownload,
    /// Transcribe `mic.wav` and `system.wav` in `dir`.
    Transcribe {
        dir: PathBuf,
        precision: Precision,
        threads: usize,
        text: TextOptions,
    },
    /// Stops the running transcription (not the ones waiting).
    CancelTranscribe,
    /// Frees the loaded model and deletes the model folder of `precision`.
    DeleteModel {
        precision: Precision,
    },
    /// Follows the files of a recording that is still being written and reports lines as they
    /// are recognized, reading new audio every `poll_ms`. Queued like a job; ends at `LiveStop`.
    LiveStart {
        dir: PathBuf,
        precision: Precision,
        threads: usize,
        poll_ms: u64,
        /// Also report the segment that is still being spoken (`Event::LivePartial`).
        partial: bool,
        text: TextOptions,
    },
    /// Ends the live run (or removes it from the queue). Does not touch the files.
    LiveStop,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineEvent {
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Debug)]
pub enum Event {
    Model(ModelStatus),
    Engine(EngineEvent),
    Progress {
        done_secs: f32,
        total_secs: f32,
        dir: PathBuf,
    },
    Done {
        dir: PathBuf,
        transcript: Transcript,
    },
    Cancelled {
        dir: PathBuf,
    },
    /// A line recognized during a live run.
    LiveLine {
        dir: PathBuf,
        line: Line,
    },
    /// The preview of the segment `track` is speaking now, or `None` when it is withdrawn
    /// (the segment became a `LiveLine`, or the track went quiet).
    LivePartial {
        dir: PathBuf,
        track: Track,
        line: Option<Line>,
    },
    /// The live run for `dir` is over (stopped, failed, or never started: no model).
    LiveEnded {
        dir: PathBuf,
    },
    /// A job is waiting: `position` 1 is the next to run. The Recorder only shows the running
    /// job, so nothing reads the fields yet; the tests do.
    #[allow(dead_code)]
    Queued {
        dir: PathBuf,
        position: usize,
    },
    Error {
        dir: Option<PathBuf>,
        message: String,
    },
}

pub struct AsrConfig {
    /// The recognizer is freed after this long without a job.
    pub idle_unload: Duration,
    /// Where the model files are downloaded from.
    pub base_url: String,
    /// Folder that holds the model folders; `None` = the user's data folder.
    pub models_base: Option<PathBuf>,
    /// Names written in front of the mic and the system lines of `transcript.txt`.
    pub labels: (String, String),
}

impl Default for AsrConfig {
    fn default() -> Self {
        Self {
            idle_unload: Duration::from_secs(300),
            base_url: model::BASE_URL.to_owned(),
            models_base: None,
            labels: ("You".to_owned(), "Others".to_owned()),
        }
    }
}

/// Makes the recognizer for a model folder; replaced by a fake in tests.
pub trait RecognizerFactory: Send + Sync {
    fn load(&self, dir: &Path, threads: usize) -> Result<Box<dyn Recognizer>, String>;
}

/// The real model on ONNX Runtime.
pub struct OrtFactory;

impl RecognizerFactory for OrtFactory {
    fn load(&self, dir: &Path, threads: usize) -> Result<Box<dyn Recognizer>, String> {
        OrtRecognizer::load(dir, threads).map(|r| Box::new(r) as Box<dyn Recognizer>)
    }
}

/// The UI's side of the worker. Dropping it cancels what runs and stops the thread.
pub struct AsrHandle {
    tx: Option<Sender<Command>>,
    rx: Receiver<Event>,
    cancel_job: Arc<AtomicBool>,
    cancel_download: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl AsrHandle {
    pub fn spawn(factory: Arc<dyn RecognizerFactory>, config: AsrConfig) -> AsrHandle {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ev_tx, ev_rx) = mpsc::channel();
        let cancel_job = Arc::new(AtomicBool::new(false));
        let cancel_download = Arc::new(AtomicBool::new(false));
        let worker = Worker {
            io: Io {
                rx: cmd_rx,
                ev: ev_tx,
                cfg: config,
                cancel_job: cancel_job.clone(),
                cancel_download: cancel_download.clone(),
                queue: RefCell::new(VecDeque::new()),
                running: RefCell::new(None),
                live: RefCell::new(None),
                live_stop: Cell::new(false),
                closed: Cell::new(false),
            },
            factory,
            loaded: None,
        };
        let join = std::thread::Builder::new()
            .name("asr-worker".into())
            .spawn(move || {
                lower_priority();
                worker.run();
            })
            .expect("spawn asr worker thread");
        AsrHandle {
            tx: Some(cmd_tx),
            rx: ev_rx,
            cancel_job,
            cancel_download,
            join: Some(join),
        }
    }

    /// Cancel commands act at once (the worker is busy with the job they stop); the others
    /// are queued for it.
    pub fn send(&self, c: Command) {
        match c {
            Command::CancelTranscribe => self.cancel_job.store(true, Ordering::SeqCst),
            Command::CancelDownload => self.cancel_download.store(true, Ordering::SeqCst),
            other => {
                if let Some(tx) = &self.tx {
                    let _ = tx.send(other);
                }
            }
        }
    }

    /// Everything the worker reported since the last call.
    pub fn try_events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

impl Drop for AsrHandle {
    fn drop(&mut self) {
        self.cancel_job.store(true, Ordering::SeqCst);
        self.cancel_download.store(true, Ordering::SeqCst);
        self.tx.take(); // disconnects the worker, which stops after the current step
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Below-normal priority for the worker thread, so a transcription does not slow down the
/// window or a recording that is running at the same time.
#[cfg(windows)]
fn lower_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: the pseudo handle of the current thread is always valid; failure is harmless.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

/// Local time as `YYYY-MM-DDTHH:MM:SS` (no zone offset).
#[cfg(windows)]
fn local_iso() -> String {
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime fills the zeroed struct we pass; it cannot fail.
    let t: SYSTEMTIME = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

#[cfg(not(windows))]
fn local_iso() -> String {
    "1970-01-01T00:00:00".to_string()
}

/// How the text is written: the model's language and the user's word fixes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextOptions {
    pub language: Language,
    pub glossary: Glossary,
}

impl TextOptions {
    /// `line` with the word fixes applied; `None` when nothing is left of it.
    fn fix(&self, line: Line) -> Option<Line> {
        let text = self.glossary.apply(&line.text);
        let text = text.trim();
        (!text.is_empty()).then(|| Line {
            text: text.to_owned(),
            ..line
        })
    }
}

struct Job {
    dir: PathBuf,
    precision: Precision,
    threads: usize,
    text: TextOptions,
}

struct LiveJob {
    dir: PathBuf,
    precision: Precision,
    threads: usize,
    poll: Duration,
    partial: bool,
    text: TextOptions,
}

enum Work {
    Transcribe(Job),
    Download(Precision),
    Delete(Precision),
    Live(LiveJob),
}

/// What the worker shares with the code running a job: the channels, the queue and the flags.
/// Everything is behind `&self` so several closures of one job can use it.
struct Io {
    rx: Receiver<Command>,
    ev: Sender<Event>,
    cfg: AsrConfig,
    cancel_job: Arc<AtomicBool>,
    cancel_download: Arc<AtomicBool>,
    queue: RefCell<VecDeque<Work>>,
    /// The folder of the running transcription, if one runs.
    running: RefCell<Option<PathBuf>>,
    /// The folder of the running live transcription, if one runs, and whether it was told to stop.
    live: RefCell<Option<PathBuf>>,
    live_stop: Cell<bool>,
    /// The handle was dropped: stop as soon as possible.
    closed: Cell<bool>,
}

impl Io {
    fn emit(&self, e: Event) {
        let _ = self.ev.send(e);
    }

    fn model_dir(&self, p: Precision) -> PathBuf {
        match &self.cfg.models_base {
            Some(base) => model::model_dir_in(base, p),
            None => model::model_dir(p),
        }
    }

    fn stop(&self) -> bool {
        self.cancel_job.load(Ordering::SeqCst) || self.closed.get()
    }

    fn error(&self, dir: &Path, message: impl Into<String>) {
        self.emit(Event::Error {
            dir: Some(dir.to_path_buf()),
            message: message.into(),
        });
    }

    fn accept(&self, cmd: Command) {
        match cmd {
            Command::CheckModel { precision } => {
                let status = model::status(&self.model_dir(precision), precision);
                self.emit(Event::Model(status));
            }
            Command::DownloadModel { precision } => {
                let queued = self
                    .queue
                    .borrow()
                    .iter()
                    .any(|w| matches!(w, Work::Download(p) if *p == precision));
                if !queued {
                    self.queue.borrow_mut().push_back(Work::Download(precision));
                }
            }
            Command::DeleteModel { precision } => {
                let queued = self
                    .queue
                    .borrow()
                    .iter()
                    .any(|w| matches!(w, Work::Delete(p) if *p == precision));
                if !queued {
                    self.queue.borrow_mut().push_back(Work::Delete(precision));
                }
            }
            Command::Transcribe {
                dir,
                precision,
                threads,
                text,
            } => {
                let running_here = self.running.borrow().as_deref() == Some(dir.as_path());
                let queued_here = self
                    .queue
                    .borrow()
                    .iter()
                    .any(|w| matches!(w, Work::Transcribe(j) if j.dir == dir));
                if running_here || queued_here {
                    return;
                }
                self.queue.borrow_mut().push_back(Work::Transcribe(Job {
                    dir: dir.clone(),
                    precision,
                    threads,
                    text,
                }));
                // The running job does not count as waiting.
                let ahead =
                    self.queue.borrow().len() - usize::from(self.running.borrow().is_none());
                if ahead >= 1 {
                    self.emit(Event::Queued {
                        dir,
                        position: ahead,
                    });
                }
            }
            Command::LiveStart {
                dir,
                precision,
                threads,
                poll_ms,
                partial,
                text,
            } => {
                let running_here = self.live.borrow().as_deref() == Some(dir.as_path());
                let queued_here = self
                    .queue
                    .borrow()
                    .iter()
                    .any(|w| matches!(w, Work::Live(j) if j.dir == dir));
                if running_here || queued_here {
                    return;
                }
                self.queue.borrow_mut().push_back(Work::Live(LiveJob {
                    dir,
                    precision,
                    threads,
                    poll: Duration::from_millis(poll_ms.max(10)),
                    partial,
                    text,
                }));
            }
            Command::LiveStop => {
                let mut removed = Vec::new();
                self.queue.borrow_mut().retain(|w| match w {
                    Work::Live(j) => {
                        removed.push(j.dir.clone());
                        false
                    }
                    _ => true,
                });
                for dir in removed {
                    self.emit(Event::LiveEnded { dir });
                }
                if self.live.borrow().is_some() {
                    self.live_stop.set(true);
                }
            }
            // The handle acts on these itself.
            Command::CancelTranscribe | Command::CancelDownload => {}
        }
    }

    /// Takes in the commands that arrived meanwhile.
    fn pump(&self) {
        loop {
            match self.rx.try_recv() {
                Ok(c) => self.accept(c),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.closed.set(true);
                    break;
                }
            }
        }
    }
}

struct Loaded {
    rec: Box<dyn Recognizer>,
    precision: Precision,
    threads: usize,
    last_used: Instant,
}

struct Worker {
    io: Io,
    factory: Arc<dyn RecognizerFactory>,
    loaded: Option<Loaded>,
}

impl Worker {
    fn run(mut self) {
        loop {
            if self.io.closed.get() {
                break;
            }
            let next = self.io.queue.borrow_mut().pop_front();
            if let Some(work) = next {
                match work {
                    Work::Transcribe(job) => self.transcribe(&job),
                    Work::Download(p) => self.download(p),
                    Work::Delete(p) => self.delete(p),
                    Work::Live(job) => self.live(&job),
                }
                continue;
            }
            let wait = match &self.loaded {
                Some(l) => self
                    .io
                    .cfg
                    .idle_unload
                    .saturating_sub(l.last_used.elapsed()),
                None => Duration::from_secs(3600),
            };
            match self.io.rx.recv_timeout(wait) {
                Ok(c) => {
                    self.io.accept(c);
                    self.io.pump();
                }
                Err(RecvTimeoutError::Timeout) => {
                    let idle = self.io.cfg.idle_unload;
                    if self
                        .loaded
                        .as_ref()
                        .is_some_and(|l| l.last_used.elapsed() >= idle)
                    {
                        self.loaded = None; // frees the model's memory
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn download(&self, p: Precision) {
        let io = &self.io;
        io.cancel_download.store(false, Ordering::SeqCst);
        let total: u64 = model::manifest(p).iter().map(|f| f.size).sum();
        io.emit(Event::Model(ModelStatus::Downloading { done: 0, total }));
        let last = Cell::new(Instant::now());
        let result = download::download(
            &io.model_dir(p),
            p,
            &io.cfg.base_url,
            &|done, total| {
                io.pump();
                if done >= total {
                    io.emit(Event::Model(ModelStatus::Verifying));
                } else if last.get().elapsed() >= DOWNLOAD_EVENT_EVERY {
                    last.set(Instant::now());
                    io.emit(Event::Model(ModelStatus::Downloading { done, total }));
                }
            },
            &io.cancel_download,
        );
        io.emit(Event::Model(match result {
            Ok(()) => ModelStatus::Ready,
            Err(e) => ModelStatus::Failed(e),
        }));
    }

    fn delete(&mut self, p: Precision) {
        // The model files stay open while it is loaded; release them first.
        self.loaded = None;
        let dir = self.io.model_dir(p);
        let event = match std::fs::remove_dir_all(&dir) {
            Ok(()) => ModelStatus::Missing,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => ModelStatus::Missing,
            Err(e) => ModelStatus::Failed(format!("Cannot delete {}: {e}", dir.display())),
        };
        self.io.emit(Event::Model(event));
    }

    /// The recognizer for `precision` and `threads`: the one kept from before, or a newly loaded
    /// one. `None` after the failure was reported for `dir`.
    fn acquire(&mut self, dir: &Path, precision: Precision, threads: usize) -> Option<Loaded> {
        match self.loaded.take() {
            Some(l) if l.precision == precision && l.threads == threads => Some(l),
            old => {
                drop(old); // free the memory of a model of another precision first
                self.io.emit(Event::Engine(EngineEvent::Loading));
                match self.factory.load(&self.io.model_dir(precision), threads) {
                    Ok(rec) => {
                        self.io.emit(Event::Engine(EngineEvent::Ready));
                        Some(Loaded {
                            rec,
                            precision,
                            threads,
                            last_used: Instant::now(),
                        })
                    }
                    Err(e) => {
                        self.io.emit(Event::Engine(EngineEvent::Failed(e.clone())));
                        self.io.error(dir, e);
                        None
                    }
                }
            }
        }
    }

    fn live(&mut self, job: &LiveJob) {
        self.io.live_stop.set(false);
        *self.io.live.borrow_mut() = Some(job.dir.clone());
        self.run_live(job);
        *self.io.live.borrow_mut() = None;
        if !self.io.closed.get() {
            self.io.emit(Event::LiveEnded {
                dir: job.dir.clone(),
            });
        }
    }

    /// Follows the files of `job.dir` until told to stop, the handle is dropped, or something
    /// fails. Lines are reported as they are recognized; nothing is written to disk.
    fn run_live(&mut self, job: &LiveJob) {
        // No model, no live lines, and no error: the app shows "waiting for the model" as usual.
        if model::status(&self.io.model_dir(job.precision), job.precision) != ModelStatus::Ready {
            return;
        }
        let Some(mut loaded) = self.acquire(&job.dir, job.precision, job.threads) else {
            return;
        };
        loaded.rec.set_language(job.text.language);
        let io = &self.io;
        let mut pending = vec![(Track::Mic, "mic.wav"), (Track::System, "system.wav")];
        let mut tracks: Vec<LiveTrack> = Vec::new();
        'run: loop {
            io.pump();
            if io.closed.get() || io.live_stop.get() {
                break;
            }
            // A file that does not exist yet (or has no header yet) is tried again next time.
            pending.retain(|(track, name)| {
                let path = job.dir.join(name);
                if !path.is_file() {
                    return true;
                }
                match LiveTrack::open(&path, *track) {
                    Ok(t) => {
                        tracks.push(if job.partial { t.with_partial() } else { t });
                        false
                    }
                    Err(_) => true,
                }
            });
            let mut lines = Vec::new();
            for t in &mut tracks {
                if let Err(e) = t.poll(loaded.rec.as_mut(), &mut lines) {
                    io.error(&job.dir, e);
                    break 'run;
                }
            }
            lines.sort_by_key(|l| l.start_ms);
            for line in lines.into_iter().filter_map(|l| job.text.fix(l)) {
                io.emit(Event::LiveLine {
                    dir: job.dir.clone(),
                    line,
                });
            }
            // After the lines, so a preview is withdrawn after the line that replaces it.
            for t in &mut tracks {
                if let Some(line) = t.take_partial_change() {
                    io.emit(Event::LivePartial {
                        dir: job.dir.clone(),
                        track: t.track(),
                        line: line.and_then(|l| job.text.fix(l)),
                    });
                }
            }
            let until = Instant::now() + job.poll;
            loop {
                io.pump();
                if io.closed.get() || io.live_stop.get() {
                    break 'run;
                }
                let left = until.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    break;
                }
                std::thread::sleep(left.min(Duration::from_millis(10)));
            }
        }
        loaded.last_used = Instant::now();
        self.loaded = Some(loaded);
    }

    fn transcribe(&mut self, job: &Job) {
        let io = &self.io;
        io.cancel_job.store(false, Ordering::SeqCst);
        *io.running.borrow_mut() = Some(job.dir.clone());
        self.run_job(job);
        *self.io.running.borrow_mut() = None;
    }

    fn run_job(&mut self, job: &Job) {
        let io = &self.io;
        let dir = job.dir.as_path();
        let model_dir = io.model_dir(job.precision);
        if model::status(&model_dir, job.precision) != ModelStatus::Ready {
            return io.error(dir, "Model is not installed");
        }

        // The recordings: every present track must be readable before any work is done, so a
        // damaged file never ends in a transcript of half the meeting.
        let mut tracks: Vec<(Track, &str, f32)> = Vec::new();
        for (track, name) in [(Track::Mic, "mic.wav"), (Track::System, "system.wav")] {
            let path = dir.join(name);
            if !path.is_file() {
                continue;
            }
            match WavReader::open(&path) {
                Ok(r) => tracks.push((track, name, r.total_frames() as f32 / r.rate() as f32)),
                Err(e) => return io.error(dir, format!("Cannot read {name}: {e}")),
            }
        }
        if tracks.is_empty() {
            return io.error(dir, format!("No recordings found in {}", dir.display()));
        }
        let total_secs: f32 = tracks.iter().map(|t| t.2).sum();

        // The model: loaded for the first job, kept for the next ones.
        let Some(mut loaded) = self.acquire(dir, job.precision, job.threads) else {
            return;
        };
        loaded.rec.set_language(job.text.language);
        let io = &self.io;
        let outcome = Self::read_tracks(io, dir, &tracks, total_secs, loaded.rec.as_mut());
        loaded.last_used = Instant::now();
        self.loaded = Some(loaded);
        let io = &self.io;
        let lines = match outcome {
            Ok(Some(lines)) => lines.into_iter().filter_map(|l| job.text.fix(l)).collect(),
            Ok(None) => {
                if !io.closed.get() {
                    io.emit(Event::Cancelled {
                        dir: dir.to_path_buf(),
                    });
                }
                return;
            }
            Err(message) => return io.error(dir, message),
        };

        let transcript = Transcript {
            model: ModelInfo {
                name: MODEL_NAME.to_owned(),
                revision: model::REVISION.to_owned(),
                precision: job.precision.as_str().to_owned(),
            },
            created: local_iso(),
            lines,
        };
        let (you, others) = &io.cfg.labels;
        match transcript::write(dir, &transcript, you, others) {
            Ok(()) => io.emit(Event::Done {
                dir: dir.to_path_buf(),
                transcript,
            }),
            Err(e) => io.error(dir, format!("Cannot save the transcript: {e}")),
        }
    }

    /// Recognizes every track. `Ok(None)` = cancelled; `Err` = a file or the model failed.
    fn read_tracks(
        io: &Io,
        dir: &Path,
        tracks: &[(Track, &str, f32)],
        total_secs: f32,
        rec: &mut dyn Recognizer,
    ) -> Result<Option<Vec<Line>>, String> {
        let progress = |done: f32| {
            io.emit(Event::Progress {
                done_secs: done.min(total_secs),
                total_secs,
                dir: dir.to_path_buf(),
            });
        };
        progress(0.0);
        let mut per_track: Vec<Vec<Line>> = Vec::new();
        let mut done_before = 0.0f32;
        for (track, name, secs) in tracks {
            let failure: RefCell<Option<String>> = RefCell::new(None);
            let mut lines = Vec::new();
            let result = segment_file(
                &dir.join(name),
                &mut |start, seg| {
                    io.pump();
                    if io.stop() || failure.borrow().is_some() {
                        return;
                    }
                    match rec.recognize(seg) {
                        Ok(text) if text.is_empty() => {}
                        Ok(text) => lines.push(Line {
                            track: *track,
                            start_ms: start as u64 * 1000 / RATE,
                            end_ms: (start + seg.len()) as u64 * 1000 / RATE,
                            text,
                        }),
                        Err(e) => *failure.borrow_mut() = Some(format!("Recognition failed: {e}")),
                    }
                },
                &mut |done, _| {
                    io.pump();
                    progress(done_before + done);
                    !io.stop() && failure.borrow().is_none()
                },
            );
            if let Err(e) = result {
                return Err(format!("Cannot read {name}: {e}"));
            }
            if io.stop() {
                return Ok(None);
            }
            if let Some(f) = failure.into_inner() {
                return Err(f);
            }
            done_before += secs;
            per_track.push(lines);
        }
        let mut per_track = per_track.into_iter();
        let (mut mic, mut system) = (Vec::new(), Vec::new());
        for (track, _, _) in tracks {
            let lines = per_track.next().unwrap_or_default();
            match track {
                Track::Mic => mic = lines,
                Track::System => system = lines,
            }
        }
        Ok(Some(transcript::merge(mic, system)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::model::{self, ModelStatus, Precision};
    use crate::asr::transcript::Track;
    use crate::audio::wav::WavWriter;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Instant;

    // ---- fakes -----------------------------------------------------------------------

    #[derive(Default)]
    struct FakeFactory {
        loads: AtomicU32,
        calls: Arc<AtomicU32>,
        fail_load: Option<String>,
        fail_recognize: Option<String>,
        delay: Duration,
        /// Recognizers dropped so far.
        drops: Arc<AtomicU32>,
        /// Every language a recognizer was set to, in order.
        languages: Arc<std::sync::Mutex<Vec<Language>>>,
    }

    struct CountingRecognizer {
        calls: Arc<AtomicU32>,
        fail: Option<String>,
        delay: Duration,
        drops: Arc<AtomicU32>,
        languages: Arc<std::sync::Mutex<Vec<Language>>>,
    }

    impl Drop for CountingRecognizer {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Recognizer for CountingRecognizer {
        fn recognize(&mut self, _samples: &[f32]) -> Result<String, String> {
            std::thread::sleep(self.delay);
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            match &self.fail {
                Some(e) => Err(e.clone()),
                None => Ok(format!("call{n}")),
            }
        }

        fn set_language(&mut self, language: Language) {
            self.languages.lock().unwrap().push(language);
        }
    }

    impl RecognizerFactory for FakeFactory {
        fn load(&self, _dir: &Path, _threads: usize) -> Result<Box<dyn Recognizer>, String> {
            self.loads.fetch_add(1, Ordering::SeqCst);
            if let Some(e) = &self.fail_load {
                return Err(e.clone());
            }
            Ok(Box::new(CountingRecognizer {
                calls: self.calls.clone(),
                fail: self.fail_recognize.clone(),
                delay: self.delay,
                drops: self.drops.clone(),
                languages: self.languages.clone(),
            }))
        }
    }

    // ---- audio -----------------------------------------------------------------------

    fn speech(secs: f32) -> Vec<i16> {
        (0..(secs * 16_000.0) as usize)
            .map(|i| (0.3 * 32_767.0 * (i as f32 * 0.07).sin()) as i16)
            .collect()
    }

    fn quiet(secs: f32) -> Vec<i16> {
        vec![0; (secs * 16_000.0) as usize]
    }

    fn write_wav(path: &Path, parts: &[Vec<i16>], finish: bool) {
        let file = std::fs::File::create(path).unwrap();
        let mut w = WavWriter::new(file, 16_000).unwrap();
        for p in parts {
            w.write_samples(p).unwrap();
        }
        if finish {
            w.finish().unwrap();
        }
    }

    // ---- rig -------------------------------------------------------------------------

    struct Rig {
        h: Option<AsrHandle>,
        factory: Arc<FakeFactory>,
        events: Vec<Event>,
        seen: usize,
        base: PathBuf,
    }

    fn install_fake_model(models: &Path, p: Precision) {
        let dir = model::model_dir_in(models, p);
        std::fs::create_dir_all(&dir).unwrap();
        let (encoder, data) = match p {
            Precision::Int8 => ("encoder.int8.onnx", "encoder.int8.onnx.data"),
            Precision::Fp32 => ("encoder.onnx", "encoder.onnx.data"),
        };
        for f in [encoder, "decoder_joint.onnx", "tokens.txt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        std::fs::write(dir.join(data), b"weights").unwrap();
        model::mark_ready(&dir, data).unwrap();
    }

    impl Rig {
        fn new() -> Rig {
            Rig::with(FakeFactory::default(), Duration::from_secs(300), true)
        }

        fn with(factory: FakeFactory, idle_unload: Duration, installed: bool) -> Rig {
            let base = crate::audio::temp_dir("asr-worker");
            let models = base.join("models");
            if installed {
                install_fake_model(&models, Precision::Int8);
            }
            let factory = Arc::new(factory);
            let h = AsrHandle::spawn(
                factory.clone(),
                AsrConfig {
                    idle_unload,
                    base_url: "http://127.0.0.1:9".to_owned(),
                    models_base: Some(models),
                    labels: ("You".to_owned(), "Others".to_owned()),
                },
            );
            Rig {
                h: Some(h),
                factory,
                events: Vec::new(),
                seen: 0,
                base,
            }
        }

        fn h(&self) -> &AsrHandle {
            self.h.as_ref().unwrap()
        }

        /// A session folder with the given tracks (`None` = no file).
        fn session(
            &self,
            name: &str,
            mic: Option<&[Vec<i16>]>,
            system: Option<&[Vec<i16>]>,
        ) -> PathBuf {
            let dir = self.base.join("sessions").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            if let Some(m) = mic {
                write_wav(&dir.join("mic.wav"), m, true);
            }
            if let Some(s) = system {
                write_wav(&dir.join("system.wav"), s, true);
            }
            dir
        }

        fn transcribe(&self, dir: &Path) {
            self.transcribe_with(dir, 4);
        }

        fn transcribe_with(&self, dir: &Path, threads: usize) {
            self.transcribe_text(dir, threads, TextOptions::default());
        }

        fn transcribe_text(&self, dir: &Path, threads: usize, text: TextOptions) {
            self.h().send(Command::Transcribe {
                dir: dir.to_path_buf(),
                precision: Precision::Int8,
                threads,
                text,
            });
        }

        fn pull(&mut self) {
            let more = self.h().try_events();
            self.events.extend(more);
        }

        /// Waits (max 10 s) for an event after the last one waited for.
        fn wait(&mut self, what: &str, f: impl Fn(&Event) -> bool) -> Event {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                self.pull();
                if let Some(i) = self.events[self.seen..].iter().position(&f) {
                    let ev = self.events[self.seen + i].clone();
                    self.seen += i + 1;
                    return ev;
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out waiting for {what}; got {:?}",
                    self.events
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        fn wait_done(&mut self) -> (PathBuf, Transcript) {
            match self.wait("Done", |e| matches!(e, Event::Done { .. })) {
                Event::Done { dir, transcript } => (dir, transcript),
                _ => unreachable!(),
            }
        }

        fn wait_error(&mut self) -> (Option<PathBuf>, String) {
            match self.wait("Error", |e| matches!(e, Event::Error { .. })) {
                Event::Error { dir, message } => (dir, message),
                _ => unreachable!(),
            }
        }

        fn engine_events(&self) -> Vec<&EngineEvent> {
            self.events
                .iter()
                .filter_map(|e| match e {
                    Event::Engine(x) => Some(x),
                    _ => None,
                })
                .collect()
        }
    }

    fn no_transcript(dir: &Path) {
        assert!(
            !dir.join("transcript.json").exists(),
            "transcript.json exists"
        );
        assert!(
            !dir.join("transcript.txt").exists(),
            "transcript.txt exists"
        );
        let tmp: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "tmp"))
            .collect();
        assert!(tmp.is_empty(), "{tmp:?}");
    }

    // ---- tests -----------------------------------------------------------------------

    #[test]
    fn both_tracks_are_merged_by_start_time() {
        let mut rig = Rig::new();
        // mic speaks at 0 s and 5 s, system at 2.5 s
        let dir = rig.session(
            "both",
            Some(&[speech(2.0), quiet(3.0), speech(2.0)]),
            Some(&[quiet(2.5), speech(2.0), quiet(2.5)]),
        );
        rig.transcribe(&dir);
        let (done_dir, t) = rig.wait_done();
        assert_eq!(done_dir, dir);
        let got: Vec<(Track, &str)> = t.lines.iter().map(|l| (l.track, l.text.as_str())).collect();
        // the mic track is read first: its segments are call1, call2; the system's is call3
        assert_eq!(
            got,
            vec![
                (Track::Mic, "call1"),
                (Track::System, "call3"),
                (Track::Mic, "call2")
            ]
        );
        for (line, want) in t.lines.iter().zip([0u64, 2250, 4750]) {
            assert!(
                line.start_ms.abs_diff(want) <= 60,
                "{} vs {want}",
                line.start_ms
            );
            assert!(line.end_ms > line.start_ms);
        }
        assert!(dir.join("transcript.json").is_file());
        let txt = std::fs::read_to_string(dir.join("transcript.txt")).unwrap();
        assert!(txt.contains("You: call1"), "{txt}");
        assert!(txt.contains("Others: call3"), "{txt}");
        assert_eq!(t.model.precision, "int8");
        assert_eq!(t.model.revision, model::REVISION);
    }

    #[test]
    fn a_session_with_one_track_is_transcribed() {
        let mut rig = Rig::new();
        let dir = rig.session("one", Some(&[speech(2.0)]), None);
        rig.transcribe(&dir);
        let (_, t) = rig.wait_done();
        assert_eq!(t.lines.len(), 1);
        assert_eq!(t.lines[0].track, Track::Mic);
    }

    #[test]
    fn a_folder_without_recordings_is_an_error() {
        let mut rig = Rig::new();
        let dir = rig.session("none", None, None);
        rig.transcribe(&dir);
        let (d, msg) = rig.wait_error();
        assert_eq!(d, Some(dir.clone()));
        assert!(msg.contains("No recordings"), "{msg}");
        no_transcript(&dir);
    }

    #[test]
    fn silent_tracks_give_an_empty_transcript_without_calling_the_model() {
        let mut rig = Rig::new();
        let dir = rig.session("silent", Some(&[quiet(4.0)]), Some(&[quiet(4.0)]));
        rig.transcribe(&dir);
        let (_, t) = rig.wait_done();
        assert!(t.lines.is_empty());
        assert!(dir.join("transcript.json").is_file());
        assert!(dir.join("transcript.txt").is_file());
        assert_eq!(rig.factory.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_file_left_by_a_crash_is_still_transcribed() {
        let mut rig = Rig::new();
        let dir = rig.session("crash", Some(&[speech(2.0)]), None);
        write_wav(&dir.join("system.wav"), &[speech(2.0)], false); // header never finished
        rig.transcribe(&dir);
        let (_, t) = rig.wait_done();
        assert_eq!(t.lines.len(), 2);
    }

    #[test]
    fn an_unreadable_track_fails_the_whole_job() {
        let mut rig = Rig::new();
        let dir = rig.session("bad", Some(&[speech(2.0)]), None);
        std::fs::write(dir.join("system.wav"), b"not a wav file at all").unwrap();
        rig.transcribe(&dir);
        let (d, msg) = rig.wait_error();
        assert_eq!(d, Some(dir.clone()));
        assert!(msg.contains("system.wav"), "{msg}");
        no_transcript(&dir);
        assert_eq!(
            rig.factory.calls.load(Ordering::SeqCst),
            0,
            "nothing was recognized"
        );
    }

    #[test]
    fn engine_loads_once_and_progress_reaches_the_total() {
        let mut rig = Rig::new();
        let a = rig.session("a", Some(&[speech(2.0)]), Some(&[speech(3.0)]));
        let b = rig.session("b", Some(&[speech(2.0)]), None);
        rig.transcribe(&a);
        rig.wait_done();
        rig.transcribe(&b);
        rig.wait_done();
        rig.pull();
        assert_eq!(
            rig.engine_events(),
            vec![&EngineEvent::Loading, &EngineEvent::Ready],
            "the model loads for the first job only"
        );
        assert_eq!(rig.factory.loads.load(Ordering::SeqCst), 1);
        let progress: Vec<(f32, f32)> = rig
            .events
            .iter()
            .filter_map(|e| match e {
                Event::Progress {
                    done_secs,
                    total_secs,
                    dir,
                } if *dir == a => Some((*done_secs, *total_secs)),
                _ => None,
            })
            .collect();
        assert!(progress.len() >= 2, "{progress:?}");
        assert_eq!(progress[0].0, 0.0);
        assert!(
            progress.windows(2).all(|w| w[0].0 <= w[1].0),
            "{progress:?}"
        );
        let (done, total) = *progress.last().unwrap();
        assert!(
            (total - 5.0).abs() < 0.01 && (done - 5.0).abs() < 0.01,
            "{progress:?}"
        );
    }

    #[test]
    fn jobs_sent_together_run_in_order_and_once_each() {
        let mut rig = Rig::with(
            FakeFactory {
                delay: Duration::from_millis(60),
                ..FakeFactory::default()
            },
            Duration::from_secs(300),
            true,
        );
        let long = [
            speech(2.0),
            quiet(2.0),
            speech(2.0),
            quiet(2.0),
            speech(2.0),
        ];
        let a = rig.session("a", Some(&long), None);
        let b = rig.session("b", Some(&[speech(2.0)]), None);
        rig.transcribe(&a);
        rig.transcribe(&b);
        rig.transcribe(&b); // again while queued: still one job
        let q = rig.wait("Queued", |e| matches!(e, Event::Queued { .. }));
        assert!(
            matches!(q, Event::Queued { ref dir, position: 1 } if *dir == b),
            "{q:?}"
        );
        let (first, _) = rig.wait_done();
        let (second, _) = rig.wait_done();
        assert_eq!((first, second), (a, b.clone()));
        std::thread::sleep(Duration::from_millis(300));
        rig.pull();
        let dones = rig
            .events
            .iter()
            .filter(|e| matches!(e, Event::Done { dir, .. } if *dir == b))
            .count();
        assert_eq!(dones, 1, "{:?}", rig.events);
    }

    #[test]
    fn cancel_stops_the_job_and_the_next_one_still_works() {
        let mut rig = Rig::with(
            FakeFactory {
                delay: Duration::from_millis(250),
                ..FakeFactory::default()
            },
            Duration::from_secs(300),
            true,
        );
        let long = [
            speech(2.0),
            quiet(2.0),
            speech(2.0),
            quiet(2.0),
            speech(2.0),
            quiet(2.0),
            speech(2.0),
        ];
        let a = rig.session("a", Some(&long), None);
        rig.transcribe(&a);
        rig.wait("engine ready", |e| {
            matches!(e, Event::Engine(EngineEvent::Ready))
        });
        std::thread::sleep(Duration::from_millis(100));
        rig.h().send(Command::CancelTranscribe);
        let c = rig.wait("Cancelled", |e| matches!(e, Event::Cancelled { .. }));
        assert!(matches!(c, Event::Cancelled { ref dir } if *dir == a));
        no_transcript(&a);

        let b = rig.session("b", Some(&[speech(2.0)]), None);
        rig.transcribe(&b);
        let (d, t) = rig.wait_done();
        assert_eq!(d, b);
        assert_eq!(t.lines.len(), 1);
    }

    #[test]
    fn dropping_the_handle_stops_a_running_job() {
        let mut rig = Rig::with(
            FakeFactory {
                delay: Duration::from_millis(250),
                ..FakeFactory::default()
            },
            Duration::from_secs(300),
            true,
        );
        let long = [
            speech(2.0),
            quiet(2.0),
            speech(2.0),
            quiet(2.0),
            speech(2.0),
            quiet(2.0),
            speech(2.0),
        ];
        let a = rig.session("a", Some(&long), None);
        rig.transcribe(&a);
        rig.wait("engine ready", |e| {
            matches!(e, Event::Engine(EngineEvent::Ready))
        });
        let start = Instant::now();
        drop(rig.h.take());
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "took {:?}",
            start.elapsed()
        );
        no_transcript(&a);
    }

    #[test]
    fn a_recognizer_error_fails_the_job_without_a_half_transcript() {
        let mut rig = Rig::with(
            FakeFactory {
                fail_recognize: Some("model exploded".to_owned()),
                ..FakeFactory::default()
            },
            Duration::from_secs(300),
            true,
        );
        let dir = rig.session("err", Some(&[speech(2.0)]), Some(&[speech(2.0)]));
        rig.transcribe(&dir);
        let (d, msg) = rig.wait_error();
        assert_eq!(d, Some(dir.clone()));
        assert!(msg.contains("model exploded"), "{msg}");
        no_transcript(&dir);
    }

    #[test]
    fn a_model_that_is_not_installed_is_an_error_before_any_loading() {
        let mut rig = Rig::with(FakeFactory::default(), Duration::from_secs(300), false);
        let dir = rig.session("nomodel", Some(&[speech(2.0)]), None);
        rig.transcribe(&dir);
        let (d, msg) = rig.wait_error();
        assert_eq!(d, Some(dir.clone()));
        assert_eq!(msg, "Model is not installed");
        rig.pull();
        assert!(rig.engine_events().is_empty());
        assert_eq!(rig.factory.loads.load(Ordering::SeqCst), 0);
        no_transcript(&dir);
    }

    #[test]
    fn a_model_that_fails_to_load_is_reported() {
        let mut rig = Rig::with(
            FakeFactory {
                fail_load: Some("bad onnx".to_owned()),
                ..FakeFactory::default()
            },
            Duration::from_secs(300),
            true,
        );
        let dir = rig.session("loadfail", Some(&[speech(2.0)]), None);
        rig.transcribe(&dir);
        let f = rig.wait("engine failed", |e| {
            matches!(e, Event::Engine(EngineEvent::Failed(_)))
        });
        assert!(matches!(f, Event::Engine(EngineEvent::Failed(ref m)) if m.contains("bad onnx")));
        let (_, msg) = rig.wait_error();
        assert!(msg.contains("bad onnx"), "{msg}");
        no_transcript(&dir);
    }

    #[test]
    fn check_model_reports_the_status_of_the_asked_precision() {
        let mut rig = Rig::new();
        rig.h().send(Command::CheckModel {
            precision: Precision::Int8,
        });
        rig.wait("ready", |e| matches!(e, Event::Model(ModelStatus::Ready)));
        rig.h().send(Command::CheckModel {
            precision: Precision::Fp32,
        });
        rig.wait("missing", |e| {
            matches!(e, Event::Model(ModelStatus::Missing))
        });
    }

    #[test]
    fn a_download_that_cannot_connect_fails_and_the_worker_stays_usable() {
        let mut rig = Rig::with(FakeFactory::default(), Duration::from_secs(300), false);
        rig.h().send(Command::DownloadModel {
            precision: Precision::Int8,
        });
        rig.wait("downloading", |e| {
            matches!(e, Event::Model(ModelStatus::Downloading { .. }))
        });
        // the first retry waits 4 s; cancelling must cut that short
        let start = Instant::now();
        rig.h().send(Command::CancelDownload);
        let f = rig.wait("failed", |e| {
            matches!(e, Event::Model(ModelStatus::Failed(_)))
        });
        assert!(
            matches!(f, Event::Model(ModelStatus::Failed(ref m)) if m == "cancelled"),
            "{f:?}"
        );
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "took {:?}",
            start.elapsed()
        );
        rig.h().send(Command::CheckModel {
            precision: Precision::Int8,
        });
        rig.wait("missing", |e| {
            matches!(e, Event::Model(ModelStatus::Missing))
        });
    }

    #[test]
    fn the_model_is_unloaded_after_the_idle_time() {
        let mut rig = Rig::with(FakeFactory::default(), Duration::from_millis(100), true);
        let a = rig.session("a", Some(&[speech(2.0)]), None);
        let b = rig.session("b", Some(&[speech(2.0)]), None);
        rig.transcribe(&a);
        rig.wait_done();
        std::thread::sleep(Duration::from_millis(500));
        rig.transcribe(&b);
        rig.wait_done();
        rig.pull();
        let loading = rig
            .engine_events()
            .iter()
            .filter(|e| ***e == EngineEvent::Loading)
            .count();
        assert_eq!(loading, 2, "{:?}", rig.events);
        assert_eq!(rig.factory.loads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn other_thread_counts_reload_the_model() {
        let mut rig = Rig::new();
        let a = rig.session("a", Some(&[speech(2.0)]), None);
        let b = rig.session("b", Some(&[speech(2.0)]), None);
        rig.transcribe_with(&a, 4);
        rig.wait_done();
        rig.transcribe_with(&b, 6);
        rig.wait_done();
        assert_eq!(rig.factory.loads.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn deleting_the_model_frees_it_and_removes_its_folder() {
        let mut rig = Rig::new();
        let dir = rig.session("a", Some(&[speech(2.0)]), None);
        rig.transcribe(&dir);
        rig.wait_done();
        let model_dir = model::model_dir_in(&rig.base.join("models"), Precision::Int8);
        assert!(model_dir.is_dir());
        assert_eq!(
            rig.factory.drops.load(Ordering::SeqCst),
            0,
            "the model is still loaded"
        );

        rig.h().send(Command::DeleteModel {
            precision: Precision::Int8,
        });
        rig.wait("missing", |e| {
            matches!(e, Event::Model(ModelStatus::Missing))
        });
        assert!(!model_dir.exists());
        assert_eq!(
            rig.factory.drops.load(Ordering::SeqCst),
            1,
            "the loaded model was released"
        );

        rig.transcribe(&dir);
        let (_, msg) = rig.wait_error();
        assert_eq!(msg, "Model is not installed");
    }

    #[test]
    fn deleting_a_model_that_is_not_there_is_not_an_error() {
        let mut rig = Rig::with(FakeFactory::default(), Duration::from_secs(300), false);
        rig.h().send(Command::DeleteModel {
            precision: Precision::Fp32,
        });
        rig.wait("missing", |e| {
            matches!(e, Event::Model(ModelStatus::Missing))
        });
    }

    #[test]
    fn deleting_one_precision_keeps_the_other() {
        let mut rig = Rig::new();
        install_fake_model(&rig.base.join("models"), Precision::Fp32);
        rig.h().send(Command::DeleteModel {
            precision: Precision::Fp32,
        });
        rig.wait("missing", |e| {
            matches!(e, Event::Model(ModelStatus::Missing))
        });
        rig.h().send(Command::CheckModel {
            precision: Precision::Int8,
        });
        rig.wait("int8 still ready", |e| {
            matches!(e, Event::Model(ModelStatus::Ready))
        });
    }

    #[test]
    fn created_time_is_an_iso_local_time() {
        let s = local_iso();
        assert_eq!(s.len(), 19, "{s}");
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[10..11], "T");
        assert_eq!(&s[13..14], ":");
    }

    // ---- live transcription -----------------------------------------------------------

    /// A track file as the recorder leaves it mid-recording (header says 0 bytes).
    fn growing_wav(path: &Path, parts: &[Vec<i16>]) {
        let file = std::fs::File::create(path).unwrap();
        let mut w = WavWriter::new(file, 16_000).unwrap();
        for p in parts {
            w.write_samples(p).unwrap();
        }
        w.flush_header().unwrap();
    }

    fn append_wav(path: &Path, parts: &[Vec<i16>]) {
        use std::io::Write as _;
        let mut f = std::fs::OpenOptions::new().append(true).open(path).unwrap();
        for p in parts {
            for v in p {
                f.write_all(&v.to_le_bytes()).unwrap();
            }
        }
        f.flush().unwrap();
    }

    impl Rig {
        fn live_dir(&self, name: &str) -> PathBuf {
            let dir = self.base.join("sessions").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        fn live_start(&self, dir: &Path) {
            self.live_start_with(dir, false);
        }

        fn live_start_partial(&self, dir: &Path) {
            self.live_start_with(dir, true);
        }

        fn live_start_with(&self, dir: &Path, partial: bool) {
            self.live_start_text(dir, partial, TextOptions::default());
        }

        fn live_start_text(&self, dir: &Path, partial: bool, text: TextOptions) {
            self.h().send(Command::LiveStart {
                dir: dir.to_path_buf(),
                precision: Precision::Int8,
                threads: 4,
                poll_ms: 20,
                partial,
                text,
            });
        }

        fn wait_line(&mut self, track: Track) -> Line {
            match self.wait(
                "a live line",
                |e| matches!(e, Event::LiveLine { line, .. } if line.track == track),
            ) {
                Event::LiveLine { line, .. } => line,
                _ => unreachable!(),
            }
        }

        fn wait_live_ended(&mut self) {
            self.wait("LiveEnded", |e| matches!(e, Event::LiveEnded { .. }));
        }

        fn live_lines(&self) -> usize {
            self.events
                .iter()
                .filter(|e| matches!(e, Event::LiveLine { .. }))
                .count()
        }
    }

    #[test]
    fn live_lines_arrive_while_the_files_grow_and_stop_ends_the_run() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-grow");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        growing_wav(&dir.join("system.wav"), &[speech(3.0), quiet(1.5)]);
        rig.live_start(&dir);
        let first_mic = rig.wait_line(Track::Mic);
        let first_system = rig.wait_line(Track::System);
        assert!(first_mic.start_ms < 1_000 && first_system.start_ms < 1_000);

        append_wav(&dir.join("mic.wav"), &[speech(2.5), quiet(1.5)]);
        let second_mic = rig.wait_line(Track::Mic);
        assert!(second_mic.start_ms > first_mic.end_ms, "{second_mic:?}");

        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
        assert_eq!(rig.live_lines(), 3, "{:?}", rig.events);
        no_transcript(&dir);
    }

    #[test]
    fn a_job_sent_during_live_runs_after_it_ends() {
        let mut rig = Rig::new();
        let live = rig.live_dir("live-first");
        growing_wav(&live.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        let other = rig.session("job-after", Some(&[speech(2.0)]), None);
        rig.live_start(&live);
        rig.wait_line(Track::Mic);
        rig.transcribe(&other);
        std::thread::sleep(Duration::from_millis(200));
        rig.pull();
        assert!(
            !rig.events.iter().any(|e| matches!(e, Event::Done { .. })),
            "the job must wait for the live run: {:?}",
            rig.events
        );
        rig.h().send(Command::LiveStop);
        let (dir, _) = rig.wait_done();
        assert_eq!(dir, other);
        let ended = rig
            .events
            .iter()
            .position(|e| matches!(e, Event::LiveEnded { .. }))
            .expect("LiveEnded");
        let done = rig
            .events
            .iter()
            .position(|e| matches!(e, Event::Done { .. }))
            .unwrap();
        assert!(ended < done);
    }

    #[test]
    fn live_with_a_missing_model_ends_without_an_error() {
        let mut rig = Rig::with(FakeFactory::default(), Duration::from_secs(300), false);
        let dir = rig.live_dir("live-no-model");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        rig.live_start(&dir);
        rig.wait_live_ended();
        assert!(
            !rig.events.iter().any(|e| matches!(e, Event::Error { .. })),
            "{:?}",
            rig.events
        );
        assert_eq!(rig.live_lines(), 0);
    }

    #[test]
    fn live_with_one_track_only_works() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-system-only");
        growing_wav(&dir.join("system.wav"), &[speech(3.0), quiet(1.5)]);
        rig.live_start(&dir);
        let line = rig.wait_line(Track::System);
        assert_eq!(line.track, Track::System);
        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
        assert_eq!(rig.live_lines(), 1);
    }

    #[test]
    fn a_track_file_that_appears_later_is_picked_up() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-late-file");
        rig.live_start(&dir);
        std::thread::sleep(Duration::from_millis(80));
        growing_wav(&dir.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        rig.wait_line(Track::Mic);
        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
    }

    #[test]
    fn a_recognizer_error_during_live_is_reported_and_ends_it() {
        let factory = FakeFactory {
            fail_recognize: Some("boom".to_owned()),
            ..FakeFactory::default()
        };
        let mut rig = Rig::with(factory, Duration::from_secs(300), true);
        let dir = rig.live_dir("live-error");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        rig.live_start(&dir);
        let (d, message) = rig.wait_error();
        assert_eq!(d.as_deref(), Some(dir.as_path()));
        assert!(message.contains("boom"), "{message}");
        rig.wait_live_ended();
    }

    #[test]
    fn live_stopped_before_it_starts_never_recognizes_anything() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-stop-first");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        rig.live_start(&dir);
        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
        std::thread::sleep(Duration::from_millis(100));
        rig.pull();
        assert_eq!(rig.live_lines(), 0);
        assert_eq!(rig.factory.calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn dropping_the_handle_stops_a_live_run() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-drop");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0), quiet(1.5)]);
        rig.live_start(&dir);
        rig.wait_line(Track::Mic);
        let t = Instant::now();
        drop(rig.h.take());
        assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    }

    impl Rig {
        fn wait_partial(&mut self, want_text: bool) -> Option<Line> {
            match self.wait(
                "a live partial",
                |e| matches!(e, Event::LivePartial { line, .. } if line.is_some() == want_text),
            ) {
                Event::LivePartial { line, .. } => line,
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn a_partial_comes_before_its_line_and_the_line_withdraws_it() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-partial");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0)]);
        rig.live_start_partial(&dir);
        let partial = rig.wait_partial(true).expect("a partial line");
        assert_eq!(partial.track, Track::Mic);
        assert!(partial.text.starts_with("call"), "{partial:?}");

        append_wav(&dir.join("mic.wav"), &[quiet(1.5)]);
        rig.wait_line(Track::Mic);
        assert_eq!(rig.wait_partial(false), None, "withdrawn after the line");
        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
    }

    #[test]
    fn without_the_partial_flag_no_partial_event_is_sent() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-no-partial");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0)]);
        rig.live_start(&dir);
        std::thread::sleep(Duration::from_millis(300));
        rig.pull();
        assert!(!rig
            .events
            .iter()
            .any(|e| matches!(e, Event::LivePartial { .. })));
        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
    }

    // ---- language and word fixes -------------------------------------------------------

    fn fixes(rules: &str, language: Language) -> TextOptions {
        TextOptions {
            language,
            glossary: crate::asr::glossary::Glossary::parse(rules),
        }
    }

    #[test]
    fn a_job_sets_the_language_and_saves_the_fixed_text() {
        let mut rig = Rig::new();
        let dir = rig.session("fixes", Some(&[speech(2.0)]), None);
        rig.transcribe_text(&dir, 4, fixes("call = โทร", Language::ThaiEnglish));
        let (_, t) = rig.wait_done();
        assert_eq!(t.lines.len(), 1);
        assert_eq!(t.lines[0].text, "โทร1");
        let json = std::fs::read_to_string(dir.join("transcript.json")).unwrap();
        assert!(json.contains("โทร1"), "{json}");
        assert_eq!(
            rig.factory.languages.lock().unwrap().last(),
            Some(&Language::ThaiEnglish)
        );
    }

    #[test]
    fn a_line_that_the_fixes_empty_is_dropped() {
        let mut rig = Rig::new();
        let dir = rig.session("fixes-empty", Some(&[speech(2.0)]), None);
        rig.transcribe_text(&dir, 4, fixes("call1 =", Language::Thai));
        let (_, t) = rig.wait_done();
        assert!(t.lines.is_empty(), "{:?}", t.lines);
    }

    #[test]
    fn live_lines_and_previews_use_the_fixes_and_the_language() {
        let mut rig = Rig::new();
        let dir = rig.live_dir("live-fixes");
        growing_wav(&dir.join("mic.wav"), &[speech(3.0)]);
        rig.live_start_text(&dir, true, fixes("call = โทร", Language::ThaiEnglish));
        let partial = rig.wait_partial(true).unwrap();
        assert!(partial.text.starts_with("โทร"), "{partial:?}");
        append_wav(&dir.join("mic.wav"), &[quiet(1.5)]);
        let line = rig.wait_line(Track::Mic);
        assert!(line.text.starts_with("โทร"), "{line:?}");
        assert_eq!(
            rig.factory.languages.lock().unwrap().last(),
            Some(&Language::ThaiEnglish)
        );
        rig.h().send(Command::LiveStop);
        rig.wait_live_ended();
    }
}
