//! Session worker: owns the WAV writers and clocks, talks to the UI through commands and events.
//!
//! One thread. It never blocks on the hardware: capture threads (behind `Backend`) push `Msg`s
//! into a channel. Before every state-changing command the worker first handles all queued
//! messages, so chunks sent before a Pause or Stop are recorded and chunks sent while paused
//! are discarded.

use std::fs::OpenOptions;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::backend::{Backend, Chunk, Msg, SourceKind, Stream};
use super::clock::TrackClock;
use super::naming;
use super::session::{
    dbfs, Command, Event, SessionState, Source, SourceStatus, StartRequest, Transition,
};
use super::wav::WavWriter;

/// Header sizes are rewritten this often (seconds of recorded audio) so a crash leaves
/// playable files.
const HEADER_FLUSH_SECS: u64 = 5;
/// A level is reported after this fraction of a second of audio (1/20 s = 50 ms).
const LEVEL_DIVISOR: u32 = 20;
/// A source that sent nothing for this long reads as silent in the meters.
const LEVEL_STALE: Duration = Duration::from_millis(300);
const SILENT_DB: f32 = -90.0;

pub struct WorkerConfig {
    /// How often a lost source or meeting app is looked for again.
    pub retry_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            retry_interval: Duration::from_secs(2),
        }
    }
}

/// The UI's side of the worker. Dropping it finalizes an active session and stops the thread.
pub struct AudioHandle {
    tx: Option<Sender<Command>>,
    rx: Receiver<Event>,
    join: Option<JoinHandle<()>>,
}

impl AudioHandle {
    pub fn spawn(backend: Arc<dyn Backend>, config: WorkerConfig) -> AudioHandle {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ev_tx, ev_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let worker = Worker {
            backend,
            cfg: config,
            cmd_rx,
            msg_tx,
            msg_rx,
            ev_tx,
            state: SessionState::Idle,
            session: None,
        };
        let join = std::thread::Builder::new()
            .name("audio-worker".into())
            .spawn(move || worker.run())
            .expect("spawn audio worker thread");
        AudioHandle {
            tx: Some(cmd_tx),
            rx: ev_rx,
            join: Some(join),
        }
    }

    pub fn send(&self, c: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }

    /// Everything the worker reported since the last call.
    pub fn try_events(&self) -> Vec<Event> {
        self.rx.try_iter().collect()
    }
}

impl Drop for AudioHandle {
    fn drop(&mut self) {
        self.tx.take(); // disconnects the worker, which finalizes and exits
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

struct Track {
    writer: WavWriter<std::fs::File>,
    clock: TrackClock,
    stream: Option<Box<dyn Stream>>,
    /// What the stream was opened as; a lost process source waits for the app, others for
    /// the device.
    kind: SourceKind,
    /// Set while the source is lost: when to try again.
    retry_at: Option<Instant>,
    last_flush: u64,
    level_db: f32,
    last_chunk: Option<Instant>,
}

impl Track {
    fn create(dir: &Path, file: &str, rate: u32, kind: SourceKind) -> io::Result<Track> {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.join(file))?;
        Ok(Track {
            writer: WavWriter::new(f, rate)?,
            clock: TrackClock::new(rate),

            stream: None,
            kind,
            retry_at: None,
            last_flush: 0,
            level_db: SILENT_DB,
            last_chunk: None,
        })
    }

    /// Writes zeros in one-second pieces so a long gap never allocates hours at once.
    fn write_zeros(&mut self, frames: u64, rate: u32) -> io::Result<()> {
        let piece = vec![0i16; rate as usize];
        let mut left = frames;
        while left > 0 {
            let n = left.min(u64::from(rate)) as usize;
            self.writer.write_samples(&piece[..n])?;
            left -= n as u64;
        }
        Ok(())
    }

    fn apply_chunk(&mut self, c: &Chunk, rate: u32) -> io::Result<()> {
        let p = self.clock.place(c.start_frame, c.samples.len());
        self.write_zeros(p.silence_frames, rate)?;
        self.writer.write_samples(&c.samples[p.skip_frames..])?;
        let written = self.clock.written();
        if written - self.last_flush >= HEADER_FLUSH_SECS * u64::from(rate) {
            self.writer.flush_header()?;
            self.last_flush = written;
        }
        Ok(())
    }

    fn level(&self) -> f32 {
        match self.last_chunk {
            Some(t) if t.elapsed() <= LEVEL_STALE => self.level_db,
            _ => SILENT_DB,
        }
    }
}

struct Session {
    dir: PathBuf,
    rate: u32,
    origin: Instant,
    system: Option<Track>,
    mic: Option<Track>,
    /// After Resume, the first chunk maps the timeline onto the end of both tracks.
    pending_rebase: bool,
    since_level: usize,
    last_secs: u64,
}

impl Session {
    fn tracks_mut(&mut self) -> impl Iterator<Item = &mut Track> {
        [self.system.as_mut(), self.mic.as_mut()]
            .into_iter()
            .flatten()
    }

    fn track_mut(&mut self, s: Source) -> Option<&mut Track> {
        match s {
            Source::System => self.system.as_mut(),
            Source::Mic => self.mic.as_mut(),
        }
    }

    fn longest(&self) -> u64 {
        [self.system.as_ref(), self.mic.as_ref()]
            .into_iter()
            .flatten()
            .map(|t| t.clock.written())
            .max()
            .unwrap_or(0)
    }

    /// Pads every track with silence up to the longest one.
    fn pad_to_common(&mut self) -> Result<(), String> {
        let rate = self.rate;
        let target = self.longest();
        let mut err = None;
        for t in self.tracks_mut() {
            let n = t.clock.tail_silence(target);
            match t.write_zeros(n, rate) {
                Ok(()) => t.clock.pad(n),
                Err(e) => {
                    err.get_or_insert(e.to_string());
                }
            }
        }
        err.map_or(Ok(()), Err)
    }

    fn stop_streams(&mut self) {
        for t in self.tracks_mut() {
            if let Some(s) = t.stream.take() {
                s.stop();
            }
        }
    }
}

struct Worker {
    backend: Arc<dyn Backend>,

    cfg: WorkerConfig,
    cmd_rx: Receiver<Command>,
    msg_tx: Sender<Msg>,
    msg_rx: Receiver<Msg>,
    ev_tx: Sender<Event>,
    state: SessionState,
    session: Option<Session>,
}

impl Worker {
    fn run(mut self) {
        loop {
            let mut busy = false;
            loop {
                match self.cmd_rx.try_recv() {
                    Ok(c) => {
                        self.handle_cmd(c);
                        busy = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.stop();
                        return;
                    }
                }
            }
            while let Ok(m) = self.msg_rx.try_recv() {
                self.handle_msg(m);
                busy = true;
            }

            self.retry_lost();
            if !busy {
                if let Ok(m) = self.msg_rx.recv_timeout(Duration::from_millis(5)) {
                    self.handle_msg(m);
                }
            }
        }
    }

    fn emit(&self, e: Event) {
        let _ = self.ev_tx.send(e);
    }

    fn transition(&mut self, t: Transition) {
        if let Some(s) = self.state.after(t) {
            self.state = s;
            self.emit(Event::State(s));
        }
    }

    fn drain_msgs(&mut self) {
        while let Ok(m) = self.msg_rx.try_recv() {
            self.handle_msg(m);
        }
    }

    fn handle_cmd(&mut self, c: Command) {
        match c {
            Command::Start(req) => self.start(req),
            Command::Pause => {
                self.drain_msgs();
                if self.state == SessionState::Recording {
                    let padded = self.session.as_mut().map(|s| s.pad_to_common());
                    if let Some(Err(e)) = padded {
                        self.fail(e);
                        return;
                    }
                    self.transition(Transition::Pause);
                }
            }
            Command::Resume => {
                self.drain_msgs();
                if self.state == SessionState::Paused {
                    if let Some(s) = self.session.as_mut() {
                        s.pending_rebase = true;
                    }
                    self.transition(Transition::Resume);
                }
            }
            Command::Stop => self.stop(),
            Command::ListDevices => {
                let (system, mic) = self.backend.list_devices();
                self.emit(Event::Devices { system, mic });
            }
        }
    }

    /// The meeting app's process when one runs (and opens), else the whole system's sound.
    fn open_system(
        &self,
        req: &StartRequest,
        origin: Instant,
    ) -> Result<(Box<dyn Stream>, SourceKind, SourceStatus), String> {
        let open = |kind: SourceKind| {
            self.backend
                .open(kind.clone(), origin, req.sample_rate, self.msg_tx.clone())
                .map(|s| (s, kind))
        };
        if let Some(pid) = self.backend.meeting_target() {
            if let Ok((s, kind)) = open(SourceKind::Process { pid }) {
                return Ok((s, kind, SourceStatus::Active));
            }
        }
        let (s, kind) = open(SourceKind::WholeSystem {
            device: req.system_device.clone(),
        })?;
        Ok((s, kind, SourceStatus::AllSystemFallback))
    }

    fn start(&mut self, req: StartRequest) {
        if !matches!(
            self.state,
            SessionState::Idle | SessionState::Saved | SessionState::Error
        ) {
            self.emit(Event::Error("A recording is already running".into()));
            return;
        }
        if !req.system && !req.mic {
            self.emit(Event::Error("Turn on at least one audio source".into()));
            return;
        }
        while self.msg_rx.try_recv().is_ok() {} // leftovers of an earlier session
        self.transition(Transition::Begin);
        let origin = Instant::now();

        let mut opened: Vec<(Source, Box<dyn Stream>, SourceKind)> = Vec::new();
        if req.system {
            match self.open_system(&req, origin) {
                Ok((s, kind, status)) => {
                    opened.push((Source::System, s, kind));
                    self.emit(Event::SourceStatus {
                        source: Source::System,
                        status,
                    });
                }
                Err(e) => self.emit(Event::SourceStatus {
                    source: Source::System,
                    status: SourceStatus::Failed(e),
                }),
            }
        } else {
            self.emit(Event::SourceStatus {
                source: Source::System,
                status: SourceStatus::Disabled,
            });
        }
        if req.mic {
            let kind = SourceKind::Mic {
                device: req.mic_device.clone(),
            };

            match self
                .backend
                .open(kind.clone(), origin, req.sample_rate, self.msg_tx.clone())
            {
                Ok(s) => {
                    opened.push((Source::Mic, s, kind));
                    self.emit(Event::SourceStatus {
                        source: Source::Mic,
                        status: SourceStatus::Active,
                    });
                }
                Err(e) => self.emit(Event::SourceStatus {
                    source: Source::Mic,
                    status: SourceStatus::Failed(e),
                }),
            }
        } else {
            self.emit(Event::SourceStatus {
                source: Source::Mic,
                status: SourceStatus::Disabled,
            });
        }
        if opened.is_empty() {
            self.emit(Event::Error("No audio source could be started".into()));
            self.transition(Transition::Fail);
            return;
        }

        let dir = match naming::session_dir(&req.base_dir, &naming::local_stamp(), &req.name) {
            Ok(d) => d,
            Err(e) => {
                for (_, s, _) in opened {
                    s.stop();
                }
                self.emit(Event::Error(format!(
                    "Cannot create the recording folder: {e}"
                )));
                self.transition(Transition::Fail);
                return;
            }
        };

        let mut system = None;
        let mut mic = None;
        let mut failure = None;
        let mut rest = opened.into_iter();

        for (source, stream, kind) in rest.by_ref() {
            let file = if source == Source::System {
                "system.wav"
            } else {
                "mic.wav"
            };

            match Track::create(&dir, file, req.sample_rate, kind) {
                Ok(mut t) => {
                    t.stream = Some(stream);
                    if source == Source::System {
                        system = Some(t);
                    } else {
                        mic = Some(t);
                    }
                }
                Err(e) => {
                    stream.stop();
                    failure = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failure {
            for (_, s, _) in rest {
                s.stop();
            }
            for t in [system, mic].into_iter().flatten() {
                if let Some(s) = t.stream {
                    s.stop();
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
            self.emit(Event::Error(format!(
                "Cannot create the recording files: {e}"
            )));
            self.transition(Transition::Fail);
            return;
        }

        self.session = Some(Session {
            dir,
            rate: req.sample_rate,
            origin,
            system,
            mic,
            pending_rebase: false,
            since_level: 0,
            last_secs: 0,
        });
        self.transition(Transition::Started);
        self.emit(Event::Elapsed(Duration::ZERO));
    }

    fn handle_msg(&mut self, m: Msg) {
        match m {
            Msg::Chunk(c) => self.handle_chunk(c),

            Msg::Lost(source) => self.handle_lost(source),
        }
    }

    fn handle_lost(&mut self, source: Source) {
        if !matches!(self.state, SessionState::Recording | SessionState::Paused) {
            return;
        }
        let retry_at = Instant::now() + self.cfg.retry_interval;
        let Some(t) = self.session.as_mut().and_then(|s| s.track_mut(source)) else {
            return;
        };
        if let Some(stream) = t.stream.take() {
            stream.stop();
        }
        t.retry_at = Some(retry_at);
        let status = match t.kind {
            SourceKind::Process { .. } => SourceStatus::WaitingForApp,
            _ => SourceStatus::DeviceLost,
        };
        self.emit(Event::SourceStatus { source, status });
    }

    /// Reopens lost sources whose retry time has come: the mic and a whole-system source on
    /// their device, a process source on whatever meeting app runs now.
    fn retry_lost(&mut self) {
        if !matches!(self.state, SessionState::Recording | SessionState::Paused) {
            return;
        }
        let now = Instant::now();
        let Some(sess) = self.session.as_mut() else {
            return;
        };
        let (rate, origin) = (sess.rate, sess.origin);
        let mut reopened = Vec::new();
        for source in [Source::System, Source::Mic] {
            let Some(t) = sess.track_mut(source) else {
                continue;
            };
            if t.retry_at.is_none_or(|at| at > now) {
                continue;
            }
            let kind = match &t.kind {
                SourceKind::Process { .. } => match self.backend.meeting_target() {
                    Some(pid) => SourceKind::Process { pid },
                    None => {
                        t.retry_at = Some(now + self.cfg.retry_interval);
                        continue;
                    }
                },
                other => other.clone(),
            };
            match self
                .backend
                .open(kind.clone(), origin, rate, self.msg_tx.clone())
            {
                Ok(stream) => {
                    t.stream = Some(stream);
                    t.kind = kind;
                    t.retry_at = None;
                    reopened.push(source);
                }
                Err(_) => t.retry_at = Some(now + self.cfg.retry_interval),
            }
        }
        for source in reopened {
            self.emit(Event::SourceStatus {
                source,
                status: SourceStatus::Active,
            });
        }
    }

    fn handle_chunk(&mut self, c: Chunk) {
        let recording = match self.state {
            SessionState::Recording => true,
            SessionState::Paused => false,
            _ => return,
        };
        let Some(sess) = self.session.as_mut() else {
            return;
        };
        let rate = sess.rate;
        match sess.track_mut(c.source) {
            Some(t) => {
                t.level_db = dbfs(&c.samples);
                t.last_chunk = Some(Instant::now());
            }
            None => return,
        }
        sess.since_level += c.samples.len();
        if recording {
            if sess.pending_rebase {
                for t in sess.tracks_mut() {
                    t.clock.resume(c.start_frame);
                }
                sess.pending_rebase = false;
            }
            if let Some(t) = sess.track_mut(c.source) {
                if let Err(e) = t.apply_chunk(&c, rate) {
                    self.fail(format!("Writing the recording failed: {e}"));
                    return;
                }
            }
        }
        self.report_progress(recording);
    }

    fn report_progress(&mut self, recording: bool) {
        let Some(sess) = self.session.as_mut() else {
            return;
        };
        let mut out = Vec::new();
        if recording {
            let secs = sess.longest() / u64::from(sess.rate);
            if secs != sess.last_secs {
                sess.last_secs = secs;
                out.push(Event::Elapsed(Duration::from_secs(secs)));
            }
        }
        if sess.since_level >= (sess.rate / LEVEL_DIVISOR) as usize {
            sess.since_level = 0;
            out.push(Event::Levels {
                system_db: sess.system.as_ref().map_or(SILENT_DB, Track::level),
                mic_db: sess.mic.as_ref().map_or(SILENT_DB, Track::level),
            });
        }
        for e in out {
            self.emit(e);
        }
    }

    /// Ends the session after an error: what was written stays playable.
    fn fail(&mut self, msg: String) {
        self.emit(Event::Error(msg));
        if let Some(sess) = self.session.take() {
            let _ = Self::finalize(sess);
        }
        self.transition(Transition::Fail);
    }

    fn stop(&mut self) {
        if !matches!(self.state, SessionState::Recording | SessionState::Paused) {
            return;
        }
        // Stop the streams first, then handle what they already queued.
        if let Some(s) = self.session.as_mut() {
            s.stop_streams();
        }
        self.drain_msgs();
        if !matches!(self.state, SessionState::Recording | SessionState::Paused) {
            return; // a write error ended the session while draining
        }
        self.transition(Transition::Stop);
        let Some(sess) = self.session.take() else {
            return;
        };
        self.transition(Transition::Finalize);
        match Self::finalize(sess) {
            Ok((dir, duration)) => {
                self.transition(Transition::Finalized);
                self.emit(Event::Saved { dir, duration });
            }
            Err(e) => {
                self.emit(Event::Error(format!("Finishing the recording failed: {e}")));
                self.transition(Transition::Fail);
            }
        }
    }

    /// Equalizes the track lengths and writes the final headers.
    fn finalize(mut sess: Session) -> Result<(PathBuf, Duration), String> {
        sess.stop_streams();
        let mut first_err = sess.pad_to_common().err();
        let rate = sess.rate;
        let frames = sess.longest();
        for t in [sess.system.take(), sess.mic.take()].into_iter().flatten() {
            if let Err(e) = t.writer.finish() {
                first_err.get_or_insert(e.to_string());
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok((
                sess.dir,
                Duration::from_secs_f64(frames as f64 / f64::from(rate)),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::backend::{FailKind, FakeBackend, SourceKind};
    use crate::audio::session::{
        Command, DeviceInfo, Event, SessionState, Source, SourceStatus, StartRequest,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const RATE: u32 = 48_000;

    struct Rig {
        h: AudioHandle,
        fake: FakeBackend,
        events: Vec<Event>,
        seen: usize,
        base: PathBuf,
    }

    impl Rig {
        fn new() -> Rig {
            let fake = FakeBackend::new();
            let h = AudioHandle::spawn(
                Arc::new(fake.clone()),
                WorkerConfig {
                    retry_interval: Duration::from_millis(50),
                },
            );
            Rig {
                h,
                fake,
                events: Vec::new(),
                seen: 0,
                base: crate::audio::temp_dir("worker"),
            }
        }

        fn req(&self, system: bool, mic: bool) -> StartRequest {
            StartRequest {
                system,
                mic,
                sample_rate: RATE,
                base_dir: self.base.clone(),
                name: "meeting".into(),
                system_device: None,
                mic_device: None,
            }
        }

        fn start(&mut self, system: bool, mic: bool) {
            let r = self.req(system, mic);
            self.h.send(Command::Start(r));
        }

        /// Waits (max 5 s) for an event after the last one waited for.
        fn wait(&mut self, what: &str, f: impl Fn(&Event) -> bool) -> Event {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                self.events.extend(self.h.try_events());
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

        fn wait_state(&mut self, s: SessionState) {
            self.wait(
                &format!("state {s:?}"),
                |e| matches!(e, Event::State(x) if *x == s),
            );
        }

        fn wait_saved(&mut self) -> PathBuf {
            match self.wait("Saved", |e| matches!(e, Event::Saved { .. })) {
                Event::Saved { dir, .. } => dir,
                _ => unreachable!(),
            }
        }

        fn drain(&mut self) {
            self.events.extend(self.h.try_events());
        }
    }

    fn tone(frames: usize, value: i16) -> Vec<i16> {
        vec![value; frames]
    }

    /// Samples of a finished WAV; checks the header sizes match the file.
    fn read_wav(path: &Path) -> Vec<i16> {
        let b = std::fs::read(path).unwrap();
        let data = u32::from_le_bytes(b[40..44].try_into().unwrap()) as usize;
        let riff = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
        assert_eq!(data, b.len() - 44, "data size field vs file length");
        assert_eq!(riff, data + 36, "riff size field");
        b[44..]
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    #[test]
    fn start_pause_resume_stop_writes_two_equal_files() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 0, tone(48_000, 1000));
        r.fake.push(Source::Mic, 0, tone(48_000, 2000));
        r.h.send(Command::Pause);
        r.wait_state(SessionState::Paused);
        r.fake.push(Source::System, 48_000, tone(48_000, 7777));
        r.fake.push(Source::Mic, 48_000, tone(48_000, 7777));
        r.h.send(Command::Resume);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 96_000, tone(48_000, 3000));
        r.fake.push(Source::Mic, 96_000, tone(48_000, 4000));
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        let sys = read_wav(&dir.join("system.wav"));
        let mic = read_wav(&dir.join("mic.wav"));
        assert_eq!(sys.len(), 96_000);
        assert_eq!(mic.len(), 96_000);
        assert!(sys[..48_000].iter().all(|&s| s == 1000));
        assert!(sys[48_000..].iter().all(|&s| s == 3000));
        assert!(mic[..48_000].iter().all(|&s| s == 2000));
        assert!(mic[48_000..].iter().all(|&s| s == 4000));
        assert!(
            !sys.contains(&7777) && !mic.contains(&7777),
            "paused audio leaked"
        );
    }

    #[test]
    fn silent_system_source_still_yields_equal_length_files() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        for i in 0..3 {
            r.fake.push(Source::Mic, i * 48_000, tone(48_000, 500));
        }
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        let sys = read_wav(&dir.join("system.wav"));
        let mic = read_wav(&dir.join("mic.wav"));
        assert_eq!(mic.len(), 144_000);
        assert_eq!(sys.len(), 144_000);
        assert!(sys.iter().all(|&s| s == 0));
        r.drain();
        assert!(!r.events.iter().any(|e| matches!(
            e,
            Event::SourceStatus {
                source: Source::System,
                status: SourceStatus::Failed(_)
            }
        )));
    }

    #[test]
    fn late_first_chunk_is_front_filled() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 24_000, tone(24_000, 5));
        r.fake.push(Source::Mic, 0, tone(48_000, 9));
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        let sys = read_wav(&dir.join("system.wav"));
        assert_eq!(sys.len(), 48_000);
        assert!(sys[..24_000].iter().all(|&s| s == 0));
        assert!(sys[24_000..].iter().all(|&s| s == 5));
    }

    #[test]
    fn both_sources_disabled_is_refused() {
        let mut r = Rig::new();
        r.start(false, false);
        r.wait("Error", |e| matches!(e, Event::Error(_)));
        r.drain();
        assert!(!r.events.iter().any(|e| matches!(
            e,
            Event::State(SessionState::Starting | SessionState::Recording)
        )));
        assert_eq!(
            std::fs::read_dir(&r.base).unwrap().count(),
            0,
            "no folder expected"
        );
    }

    #[test]
    fn one_source_failing_keeps_the_other() {
        let mut r = Rig::new();
        r.fake.fail_open(FailKind::Mic);
        r.start(true, true);
        r.wait("mic failed", |e| {
            matches!(
                e,
                Event::SourceStatus {
                    source: Source::Mic,
                    status: SourceStatus::Failed(_)
                }
            )
        });
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 0, tone(48_000, 11));
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        assert_eq!(read_wav(&dir.join("system.wav")).len(), 48_000);
        assert!(
            !dir.join("mic.wav").exists(),
            "failed source must leave no file"
        );
    }

    #[test]
    fn neither_source_starting_is_error_and_leaves_no_folder() {
        let mut r = Rig::new();
        r.fake.fail_open(FailKind::Mic);
        r.fake.fail_open(FailKind::WholeSystem);
        r.start(true, true);
        r.wait_state(SessionState::Error);
        r.drain();
        assert!(r.events.iter().any(|e| matches!(e, Event::Error(_))));
        assert_eq!(
            std::fs::read_dir(&r.base).unwrap().count(),
            0,
            "no folder expected"
        );
    }

    #[test]
    fn unwritable_dir_goes_to_error() {
        let mut r = Rig::new();
        let file = r.base.join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        let mut req = r.req(true, true);
        req.base_dir = file;
        r.h.send(Command::Start(req));
        r.wait_state(SessionState::Error);
        r.drain();
        assert!(r.events.iter().any(|e| matches!(e, Event::Error(_))));
        // Restartable afterwards.
        r.start(true, true);
        r.wait_state(SessionState::Recording);
    }

    #[test]
    fn levels_follow_chunk_loudness() {
        let mut r = Rig::new();
        r.start(false, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::Mic, 0, tone(48_000, 20_000));
        let loud = r.wait(
            "loud level",
            |e| matches!(e, Event::Levels { mic_db, .. } if *mic_db > -20.0),
        );
        if let Event::Levels { system_db, .. } = loud {
            assert_eq!(system_db, -90.0, "disabled source reports silence");
        }
        r.fake.push(Source::Mic, 48_000, tone(4_800, 0));
        r.wait(
            "silent level",
            |e| matches!(e, Event::Levels { mic_db, .. } if *mic_db == -90.0),
        );
    }

    #[test]
    fn list_devices_emits_devices_event() {
        let mut r = Rig::new();
        let dev = |id: &str, d: bool| DeviceInfo {
            id: id.into(),
            name: id.into(),
            is_default: d,
        };
        r.fake.set_devices(
            vec![dev("spk", true)],
            vec![dev("mic1", true), dev("mic2", false)],
        );
        r.h.send(Command::ListDevices);
        match r.wait("Devices", |e| matches!(e, Event::Devices { .. })) {
            Event::Devices { system, mic } => {
                assert_eq!(system.len(), 1);
                assert_eq!(mic.len(), 2);
            }
            _ => unreachable!(),
        }
    }

    fn opened_has(r: &Rig, kind: SourceKind) -> bool {
        r.fake.opened().contains(&kind)
    }
    fn whole() -> SourceKind {
        SourceKind::WholeSystem { device: None }
    }
    fn status(source: Source, status: SourceStatus) -> impl Fn(&Event) -> bool {
        move |e| matches!(e, Event::SourceStatus { source: s, status: st } if *s == source && *st == status)
    }

    #[test]
    fn no_meeting_app_falls_back_to_whole_system() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait(
            "fallback status",
            status(Source::System, SourceStatus::AllSystemFallback),
        );
        assert!(opened_has(&r, whole()));
    }

    #[test]
    fn meeting_app_found_opens_process_source() {
        let mut r = Rig::new();
        r.fake.set_target(Some(42));
        r.start(true, true);
        r.wait(
            "system active",
            status(Source::System, SourceStatus::Active),
        );
        r.wait_state(SessionState::Recording);
        assert!(opened_has(&r, SourceKind::Process { pid: 42 }));
        assert!(!opened_has(&r, whole()));
        r.drain();
        assert!(!r
            .events
            .iter()
            .any(status(Source::System, SourceStatus::AllSystemFallback)));
    }

    #[test]
    fn process_open_failure_falls_back_to_whole_system() {
        let mut r = Rig::new();
        r.fake.set_target(Some(1));
        r.fake.fail_open(FailKind::Process);
        r.start(true, true);
        r.wait(
            "fallback status",
            status(Source::System, SourceStatus::AllSystemFallback),
        );
        assert!(opened_has(&r, whole()));
    }

    #[test]
    fn fallback_does_not_switch_mid_recording() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.set_target(Some(9));
        std::thread::sleep(Duration::from_millis(300)); // six retry intervals
        assert!(!opened_has(&r, SourceKind::Process { pid: 9 }));
    }

    #[test]
    fn app_exit_waits_then_reattaches() {
        let mut r = Rig::new();
        r.fake.set_target(Some(5));
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 0, tone(48_000, 1));
        r.fake.push(Source::Mic, 0, tone(48_000, 2));
        r.fake.set_target(None);
        assert!(r.fake.lose(Source::System));
        r.wait(
            "waiting for app",
            status(Source::System, SourceStatus::WaitingForApp),
        );
        r.fake.set_target(Some(7));
        r.wait("reattached", status(Source::System, SourceStatus::Active));
        assert!(opened_has(&r, SourceKind::Process { pid: 7 }));
        r.fake.push(Source::System, 96_000, tone(48_000, 3));
        r.fake.push(Source::Mic, 48_000, tone(96_000, 4));
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        let sys = read_wav(&dir.join("system.wav"));
        let mic = read_wav(&dir.join("mic.wav"));
        assert_eq!(sys.len(), mic.len());
        assert_eq!(sys.len(), 144_000);
        assert!(
            sys[48_000..96_000].iter().all(|&s| s == 0),
            "the gap is silence"
        );
        assert!(sys[96_000..].iter().all(|&s| s == 3));
    }

    #[test]
    fn mic_lost_continues_system_and_reattaches() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 0, tone(48_000, 1));
        r.fake.push(Source::Mic, 0, tone(48_000, 2));
        assert!(r.fake.lose(Source::Mic));
        r.wait("device lost", status(Source::Mic, SourceStatus::DeviceLost));
        r.wait("mic back", status(Source::Mic, SourceStatus::Active));
        r.fake.push(Source::System, 48_000, tone(48_000, 1));
        r.fake.push(Source::Mic, 96_000, tone(48_000, 2));
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        let sys = read_wav(&dir.join("system.wav"));
        let mic = read_wav(&dir.join("mic.wav"));
        assert_eq!(sys.len(), mic.len());
        assert_eq!(mic.len(), 144_000);
    }

    #[test]
    fn stop_while_paused_finalizes_equal_length() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 0, tone(48_000, 1));
        r.fake.push(Source::Mic, 0, tone(24_000, 2));
        r.h.send(Command::Pause);
        r.wait_state(SessionState::Paused);
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        assert_eq!(read_wav(&dir.join("system.wav")).len(), 48_000);
        assert_eq!(read_wav(&dir.join("mic.wav")).len(), 48_000);
    }

    #[test]
    fn stop_while_source_lost_finalizes_equal_length() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.fail_open(FailKind::Mic); // so it cannot reattach
        r.fake.push(Source::System, 0, tone(96_000, 1));
        r.fake.push(Source::Mic, 0, tone(48_000, 2));
        assert!(r.fake.lose(Source::Mic));
        r.wait("device lost", status(Source::Mic, SourceStatus::DeviceLost));
        r.h.send(Command::Stop);
        let dir = r.wait_saved();
        let mic = read_wav(&dir.join("mic.wav"));
        assert_eq!(read_wav(&dir.join("system.wav")).len(), 96_000);
        assert_eq!(mic.len(), 96_000);
        assert!(
            mic[48_000..].iter().all(|&s| s == 0),
            "lost time is silence"
        );
    }

    #[test]
    fn drop_handle_mid_recording_finalizes_files() {
        let mut r = Rig::new();
        r.start(true, true);
        r.wait_state(SessionState::Recording);
        r.fake.push(Source::System, 0, tone(48_000, 1));
        r.fake.push(Source::Mic, 0, tone(24_000, 2));
        let Rig { h, base, .. } = r;
        drop(h);
        let session = std::fs::read_dir(&base)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(read_wav(&session.join("system.wav")).len(), 48_000);
        assert_eq!(read_wav(&session.join("mic.wav")).len(), 48_000);
    }
}
