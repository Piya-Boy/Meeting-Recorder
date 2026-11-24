//! Playback of a saved recording: the two tracks mixed and sent to the default output device.
//! See docs/superpowers/specs/2026-11-25-playback-design.md.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::asr::wavread::WavReader;

/// `mic.wav` and `system.wav` of one session read as a single mixed mono stream.
pub struct Mixer {
    tracks: Vec<WavReader>,
    rate: u32,
    total: u64,
    pos: u64,
}

impl Mixer {
    /// Opens whichever of `mic.wav` / `system.wav` exist. A folder with neither, a track that is
    /// not 16-bit mono PCM, or two tracks with different rates is an error.
    pub fn open(dir: &Path) -> io::Result<Mixer> {
        let mut tracks = Vec::new();
        for name in ["mic.wav", "system.wav"] {
            let path = dir.join(name);
            if !path.is_file() {
                continue;
            }
            tracks.push(WavReader::open(&path)?);
        }
        let Some(first) = tracks.first() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no mic.wav or system.wav in the recording folder",
            ));
        };
        let rate = first.rate();
        if tracks.iter().any(|t| t.rate() != rate) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the two tracks have different sample rates",
            ));
        }
        let total = tracks.iter().map(|t| t.total_frames()).max().unwrap_or(0);
        Ok(Mixer {
            tracks,
            rate,
            total,
            pos: 0,
        })
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn total_frames(&self) -> u64 {
        self.total
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    /// Moves to `frame` (clamped to the end).
    pub fn seek(&mut self, frame: u64) -> io::Result<()> {
        let frame = frame.min(self.total);
        for t in &mut self.tracks {
            t.seek_frame(frame)?;
        }
        self.pos = frame;
        Ok(())
    }

    /// Up to `max_frames` mixed samples in [-1, 1]; empty at the end.
    pub fn read(&mut self, max_frames: usize) -> io::Result<Vec<f32>> {
        let n = (self.total - self.pos).min(max_frames as u64) as usize;
        let mut out = vec![0.0f32; n];
        for t in &mut self.tracks {
            // A track that ends early returns fewer samples; the rest of the block stays silent.
            for (o, v) in out.iter_mut().zip(t.read_block(n)?) {
                *o += v;
            }
        }
        for o in &mut out {
            *o = o.clamp(-1.0, 1.0);
        }
        self.pos += n as u64;
        Ok(out)
    }
}

/// Where mixed samples go. One block is mono `f32`; the sink adapts it to the device. Created and
/// used on the playback thread only (COM objects are not `Send`).
pub trait Sink {
    /// Starts (or restarts) the stream.
    fn start(&mut self) -> Result<(), String>;
    /// Stops the stream; buffered frames stay until `discard`.
    fn stop(&mut self) -> Result<(), String>;
    /// Drops everything buffered (the stream must be stopped) and returns how many frames that was.
    fn discard(&mut self) -> Result<usize, String>;
    /// Frames that can be written now.
    fn space(&mut self) -> Result<usize, String>;
    /// Frames written but not yet played.
    fn padding(&mut self) -> Result<usize, String>;
    fn write(&mut self, block: &[f32]) -> Result<(), String>;
    /// Blocks until the device wants more data (or a short timeout).
    fn wait(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Resume,
    Pause,
    /// Move to this frame (clamped to the end).
    Seek(u64),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Paused,
    Playing,
    Finished,
    Failed,
}

impl State {
    fn to_u8(self) -> u8 {
        match self {
            State::Paused => 0,
            State::Playing => 1,
            State::Finished => 2,
            State::Failed => 3,
        }
    }

    fn from_u8(v: u8) -> State {
        match v {
            0 => State::Paused,
            1 => State::Playing,
            2 => State::Finished,
            _ => State::Failed,
        }
    }
}

/// What the UI sees of a player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub state: State,
    pub position_frames: u64,
    pub total_frames: u64,
    pub rate: u32,
    pub error: Option<String>,
}

struct Shared {
    state: AtomicU8,
    position: AtomicU64,
    total: AtomicU64,
    rate: AtomicU32,
    error: Mutex<Option<String>>,
}

impl Shared {
    fn new(state: State) -> Shared {
        Shared {
            state: AtomicU8::new(state.to_u8()),
            position: AtomicU64::new(0),
            total: AtomicU64::new(0),
            rate: AtomicU32::new(0),
            error: Mutex::new(None),
        }
    }

    fn set_state(&self, state: State) {
        self.state.store(state.to_u8(), Ordering::SeqCst);
    }

    fn fail(&self, message: String) {
        *self.error.lock().unwrap_or_else(|e| e.into_inner()) = Some(message);
        self.set_state(State::Failed);
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            state: State::from_u8(self.state.load(Ordering::SeqCst)),
            position_frames: self.position.load(Ordering::SeqCst),
            total_frames: self.total.load(Ordering::SeqCst),
            rate: self.rate.load(Ordering::SeqCst),
            error: self.error.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        }
    }
}

fn io_message(e: io::Error) -> String {
    format!("could not read the recording: {e}")
}

/// The playback state machine, independent of any device.
struct Playback {
    mixer: Mixer,
    sink: Box<dyn Sink>,
    state: State,
    shared: Arc<Shared>,
    /// Most frames written per step: a tenth of a second.
    block_frames: usize,
}

impl Playback {
    fn new(mixer: Mixer, sink: Box<dyn Sink>, shared: Arc<Shared>) -> Playback {
        shared.rate.store(mixer.rate(), Ordering::SeqCst);
        shared.total.store(mixer.total_frames(), Ordering::SeqCst);
        shared.position.store(0, Ordering::SeqCst);
        shared.set_state(State::Paused);
        let block_frames = (mixer.rate() / 10).max(1) as usize;
        Playback {
            mixer,
            sink,
            state: State::Paused,
            shared,
            block_frames,
        }
    }

    fn state(&self) -> State {
        self.state
    }

    fn set_state(&mut self, state: State) {
        self.state = state;
        self.shared.set_state(state);
    }

    fn fail(&mut self, message: String) {
        let _ = self.sink.stop();
        self.state = State::Failed;
        self.shared.fail(message);
    }

    /// Applies one command. A failure turns the player into `Failed`.
    fn apply(&mut self, cmd: Cmd) {
        if self.state == State::Failed {
            return;
        }
        if let Err(e) = self.apply_inner(cmd) {
            self.fail(e);
        }
    }

    fn apply_inner(&mut self, cmd: Cmd) -> Result<(), String> {
        match cmd {
            Cmd::Resume => {
                if self.state == State::Playing {
                    return Ok(());
                }
                // From the end (finished, or a seek to the very end) play again from the start.
                if self.state == State::Finished
                    || self.mixer.position() >= self.mixer.total_frames()
                {
                    self.mixer.seek(0).map_err(io_message)?;
                }
                self.fill()?;
                self.sink.start()?;
                self.set_state(State::Playing);
                self.publish()
            }
            Cmd::Pause => {
                if self.state != State::Playing {
                    return Ok(());
                }
                // What is buffered was never heard: drop it and take the mixer back by that much,
                // so Resume continues exactly where the listener stopped.
                self.sink.stop()?;
                let dropped = self.sink.discard()?;
                let heard = self.mixer.position().saturating_sub(dropped as u64);
                self.mixer.seek(heard).map_err(io_message)?;
                self.set_state(State::Paused);
                self.publish()
            }
            Cmd::Seek(frame) => {
                let playing = self.state == State::Playing;
                if playing {
                    self.sink.stop()?;
                    self.sink.discard()?;
                }
                self.mixer.seek(frame).map_err(io_message)?;
                if self.state == State::Finished
                    && self.mixer.position() < self.mixer.total_frames()
                {
                    self.set_state(State::Paused);
                }
                if playing {
                    self.fill()?;
                    self.sink.start()?;
                }
                self.publish()
            }
            Cmd::Quit => Ok(()),
        }
    }

    /// Writes up to a block of mixed audio into the free space of the sink.
    fn fill(&mut self) -> Result<(), String> {
        let n = self.sink.space()?.min(self.block_frames);
        if n == 0 {
            return Ok(());
        }
        let block = self.mixer.read(n).map_err(io_message)?;
        if !block.is_empty() {
            self.sink.write(&block)?;
        }
        Ok(())
    }

    /// The position the listener has reached: what was mixed minus what is still buffered.
    fn publish(&mut self) -> Result<(), String> {
        let buffered = if self.state == State::Playing {
            self.sink.padding()? as u64
        } else {
            0
        };
        let heard = self.mixer.position().saturating_sub(buffered);
        self.shared.position.store(heard, Ordering::SeqCst);
        Ok(())
    }

    /// One step of the loop: top up the buffer, detect the end, publish the position.
    fn tick(&mut self) {
        if self.state != State::Playing {
            return;
        }
        if let Err(e) = self.tick_inner() {
            self.fail(e);
        }
    }

    fn tick_inner(&mut self) -> Result<(), String> {
        self.fill()?;
        let at_end = self.mixer.position() >= self.mixer.total_frames();
        if at_end && self.sink.padding()? == 0 {
            self.sink.stop()?;
            self.set_state(State::Finished);
        }
        self.publish()
    }

    fn shutdown(&mut self) {
        let _ = self.sink.stop();
    }

    fn wait(&mut self) {
        self.sink.wait();
    }
}

/// Handle on a playback thread. Dropping it stops the thread and waits until it has closed the
/// files.
pub struct PlayerHandle {
    dir: PathBuf,
    tx: Sender<Cmd>,
    shared: Arc<Shared>,
    join: Option<JoinHandle<()>>,
}

type SinkFactory = Box<dyn FnOnce(u32) -> Result<Box<dyn Sink>, String> + Send>;

impl PlayerHandle {
    /// Plays the recording in `dir` through the default output device.
    pub fn start(dir: &Path) -> PlayerHandle {
        PlayerHandle::start_with(dir, |rate| {
            #[cfg(windows)]
            {
                wasapi_sink::WasapiSink::open(rate).map(|s| Box::new(s) as Box<dyn Sink>)
            }
            #[cfg(not(windows))]
            {
                let _ = rate;
                Err("playback needs Windows".to_owned())
            }
        })
    }

    /// Like `start`, with the sink made on the playback thread by `make_sink(rate)`.
    pub fn start_with(
        dir: &Path,
        make_sink: impl FnOnce(u32) -> Result<Box<dyn Sink>, String> + Send + 'static,
    ) -> PlayerHandle {
        let (tx, rx) = mpsc::channel();
        // Until the thread knows better the player counts as playing, so the bar does not flicker.
        let shared = Arc::new(Shared::new(State::Playing));
        let join = {
            let dir = dir.to_path_buf();
            let shared = shared.clone();
            let make_sink: SinkFactory = Box::new(make_sink);
            std::thread::Builder::new()
                .name("playback".into())
                .spawn(move || run(&dir, make_sink, &rx, &shared))
                .ok()
        };
        if join.is_none() {
            shared.fail("could not start the playback thread".to_owned());
        }
        PlayerHandle {
            dir: dir.to_path_buf(),
            tx,
            shared,
            join,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.shared.snapshot()
    }

    /// Stops playback and returns once the thread has ended and the files are closed.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let _ = self.tx.send(Cmd::Quit);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for PlayerHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run(dir: &Path, make_sink: SinkFactory, rx: &Receiver<Cmd>, shared: &Arc<Shared>) {
    #[cfg(windows)]
    let _ = wasapi::initialize_mta();
    run_player(dir, make_sink, rx, shared);
    #[cfg(windows)]
    wasapi::deinitialize();
}

fn run_player(dir: &Path, make_sink: SinkFactory, rx: &Receiver<Cmd>, shared: &Arc<Shared>) {
    let mixer = match Mixer::open(dir) {
        Ok(m) => m,
        Err(e) => return shared.fail(io_message(e)),
    };
    let sink = match make_sink(mixer.rate()) {
        Ok(s) => s,
        Err(e) => return shared.fail(e),
    };
    let mut pb = Playback::new(mixer, sink, shared.clone());
    pb.apply(Cmd::Resume);
    loop {
        let mut cmds = Vec::new();
        if pb.state() == State::Playing {
            pb.wait();
        } else {
            // Nothing to feed: sleep until a command comes (or a short timeout).
            match rx.recv_timeout(Duration::from_millis(20)) {
                Ok(c) => cmds.push(c),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => cmds.push(Cmd::Quit),
            }
        }
        loop {
            match rx.try_recv() {
                Ok(c) => cmds.push(c),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    cmds.push(Cmd::Quit);
                    break;
                }
            }
        }
        for c in cmds {
            if c == Cmd::Quit {
                pb.shutdown();
                return;
            }
            pb.apply(c);
        }
        pb.tick();
        if pb.state() == State::Failed {
            pb.shutdown();
            return;
        }
    }
}

#[cfg(windows)]
mod wasapi_sink {
    use super::Sink;
    use wasapi::{
        AudioClient, AudioRenderClient, DeviceEnumerator, Direction, Handle, SampleType,
        StreamMode, WaveFormat,
    };

    /// 200 ms in 100-nanosecond units.
    const BUFFER_HNS: i64 = 2_000_000;

    /// Shared-mode, event-driven render on the default output device. Windows converts our mono
    /// float stream at the file's rate to whatever the device uses (`autoconvert`).
    pub struct WasapiSink {
        client: AudioClient,
        event: Handle,
        render: AudioRenderClient,
        channels: usize,
    }

    impl WasapiSink {
        pub fn open(rate: u32) -> Result<WasapiSink, String> {
            let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
            let mut last = String::new();
            for channels in [1usize, 2] {
                let mut client = enumerator
                    .get_default_device(Direction::Render)
                    .and_then(|d| d.get_iaudioclient())
                    .map_err(|e| e.to_string())?;
                let format =
                    WaveFormat::new(32, 32, SampleType::Float, rate as usize, channels, None);
                let mode = StreamMode::EventsShared {
                    autoconvert: true,
                    buffer_duration_hns: BUFFER_HNS,
                };
                if let Err(e) = client.initialize_client(&format, Direction::Render, mode, None) {
                    last = e.to_string();
                    continue;
                }
                let event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
                let render = client.get_audiorenderclient().map_err(|e| e.to_string())?;
                return Ok(WasapiSink {
                    client,
                    event,
                    render,
                    channels,
                });
            }
            Err(last)
        }
    }

    impl Sink for WasapiSink {
        fn start(&mut self) -> Result<(), String> {
            self.client.start_stream().map_err(|e| e.to_string())
        }

        fn stop(&mut self) -> Result<(), String> {
            self.client.stop_stream().map_err(|e| e.to_string())
        }

        fn discard(&mut self) -> Result<usize, String> {
            let buffered = self
                .client
                .get_current_padding()
                .map_err(|e| e.to_string())?;
            self.client.reset_stream().map_err(|e| e.to_string())?;
            Ok(buffered as usize)
        }

        fn space(&mut self) -> Result<usize, String> {
            self.client
                .get_available_space_in_frames()
                .map(|n| n as usize)
                .map_err(|e| e.to_string())
        }

        fn padding(&mut self) -> Result<usize, String> {
            self.client
                .get_current_padding()
                .map(|n| n as usize)
                .map_err(|e| e.to_string())
        }

        fn write(&mut self, block: &[f32]) -> Result<(), String> {
            let mut bytes = Vec::with_capacity(block.len() * self.channels * 4);
            for sample in block {
                for _ in 0..self.channels {
                    bytes.extend_from_slice(&sample.to_le_bytes());
                }
            }
            self.render
                .write_to_device(&bytes, None)
                .map_err(|e| e.to_string())
        }

        fn wait(&mut self) {
            let _ = self.event.wait_for_event(100); // a timeout just means no room yet
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::wav::WavWriter;
    use std::fs;
    use std::path::PathBuf;

    fn write_wav(path: &Path, rate: u32, samples: &[i16]) {
        let f = fs::File::create(path).unwrap();
        let mut w = WavWriter::new(f, rate).unwrap();
        w.write_samples(samples).unwrap();
        w.finish().unwrap();
    }

    fn dir(tag: &str) -> PathBuf {
        crate::audio::temp_dir(tag)
    }

    fn s(v: i16) -> f32 {
        f32::from(v) / 32_768.0
    }

    fn close(a: &[f32], b: &[f32]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn mixer_sums_the_two_tracks() {
        let d = dir("mix-sum");
        write_wav(&d.join("mic.wav"), 16_000, &[1000, 2000, 3000]);
        write_wav(&d.join("system.wav"), 16_000, &[10, 20, 30]);
        let mut m = Mixer::open(&d).unwrap();
        assert_eq!((m.rate(), m.total_frames(), m.position()), (16_000, 3, 0));
        let got = m.read(10).unwrap();
        assert!(close(&got, &[s(1010), s(2020), s(3030)]), "{got:?}");
        assert!(m.read(10).unwrap().is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn mixer_clamps_loud_sums_to_one() {
        let d = dir("mix-clamp");
        write_wav(&d.join("mic.wav"), 16_000, &[30_000, -30_000]);
        write_wav(&d.join("system.wav"), 16_000, &[30_000, -30_000]);
        let got = Mixer::open(&d).unwrap().read(10).unwrap();
        assert_eq!(got, vec![1.0, -1.0]);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_shorter_track_is_silent_after_its_end() {
        let d = dir("mix-short");
        write_wav(&d.join("mic.wav"), 16_000, &[100, 200, 300, 400]);
        write_wav(&d.join("system.wav"), 16_000, &[1, 2]);
        let mut m = Mixer::open(&d).unwrap();
        assert_eq!(m.total_frames(), 4);
        let got = m.read(10).unwrap();
        assert!(close(&got, &[s(101), s(202), s(300), s(400)]), "{got:?}");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn mixer_plays_a_folder_with_only_one_track() {
        for only in ["mic.wav", "system.wav"] {
            let d = dir("mix-one");
            write_wav(&d.join(only), 48_000, &[5, 6, 7]);
            let mut m = Mixer::open(&d).unwrap();
            assert_eq!((m.rate(), m.total_frames()), (48_000, 3));
            assert!(close(&m.read(10).unwrap(), &[s(5), s(6), s(7)]), "{only}");
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn mixer_rejects_no_wav_a_bad_wav_and_mismatched_rates() {
        let d = dir("mix-bad-none");
        assert!(Mixer::open(&d).is_err(), "empty folder");

        let d2 = dir("mix-bad-garbage");
        fs::write(d2.join("mic.wav"), b"not a wav").unwrap();
        assert!(Mixer::open(&d2).is_err(), "garbage file");

        // A real header, but 2 channels: the recorder never writes that.
        let d3 = dir("mix-bad-stereo");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
        bytes.extend_from_slice(&2u16.to_le_bytes()); // channels
        bytes.extend_from_slice(&16_000u32.to_le_bytes());
        bytes.extend_from_slice(&64_000u32.to_le_bytes());
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&4u32.to_le_bytes());
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        fs::write(d3.join("mic.wav"), bytes).unwrap();
        assert!(Mixer::open(&d3).is_err(), "stereo file");

        let d4 = dir("mix-bad-rates");
        write_wav(&d4.join("mic.wav"), 16_000, &[1, 2]);
        write_wav(&d4.join("system.wav"), 48_000, &[1, 2]);
        assert!(Mixer::open(&d4).is_err(), "rates differ");

        // One good track and one broken: an error, not a silent half recording.
        let d5 = dir("mix-bad-one-of-two");
        write_wav(&d5.join("mic.wav"), 16_000, &[1, 2]);
        fs::write(d5.join("system.wav"), b"junk").unwrap();
        assert!(Mixer::open(&d5).is_err(), "one broken track");

        for d in [d, d2, d3, d4, d5] {
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn mixer_seek_and_position() {
        let d = dir("mix-seek");
        let ramp: Vec<i16> = (0..100).collect();
        write_wav(&d.join("mic.wav"), 16_000, &ramp);
        let mut m = Mixer::open(&d).unwrap();
        m.seek(50).unwrap();
        assert_eq!(m.position(), 50);
        let got = m.read(10).unwrap();
        let want: Vec<f32> = (50..60).map(s).collect();
        assert!(close(&got, &want), "{got:?}");
        assert_eq!(m.position(), 60);

        m.seek(1_000).unwrap();
        assert_eq!(m.position(), 100, "clamped to the end");
        assert!(m.read(10).unwrap().is_empty());

        m.seek(0).unwrap();
        let mut all = Vec::new();
        loop {
            let b = m.read(7).unwrap();
            if b.is_empty() {
                break;
            }
            all.extend(b);
        }
        assert_eq!(all.len(), 100);
        assert_eq!(m.position(), 100);
        let _ = fs::remove_dir_all(&d);
    }

    // ─── Playback state machine and handle (fake sink, no audio device) ──────────────────────

    use super::testing::{FakeHandle, FakeSink};

    const RATE: u32 = 16_000;

    /// A session folder with a mic-only ramp of `frames` samples (values 0..frames, frames < 32768).
    fn ramp_dir(tag: &str, frames: usize) -> (PathBuf, Vec<f32>) {
        let d = dir(tag);
        let ramp: Vec<i16> = (0..frames).map(|i| (i % 30_000) as i16).collect();
        write_wav(&d.join("mic.wav"), RATE, &ramp);
        (d, ramp.iter().map(|&v| s(v)).collect())
    }

    fn player(d: &Path, capacity: usize) -> (Playback, FakeHandle, Arc<Shared>) {
        let (sink, fake) = FakeSink::new(capacity, 480);
        let shared = Arc::new(Shared::new(State::Paused));
        let pb = Playback::new(Mixer::open(d).unwrap(), Box::new(sink), shared.clone());
        (pb, fake, shared)
    }

    /// Advances the fake device 480 frames and ticks, until `cond` holds.
    fn run_until(
        pb: &mut Playback,
        fake: &FakeHandle,
        cond: impl Fn(&Playback, &FakeHandle) -> bool,
    ) {
        for _ in 0..10_000 {
            if cond(pb, fake) {
                return;
            }
            fake.advance(480);
            pb.tick();
        }
        panic!("condition not reached; state {:?}", pb.state());
    }

    fn finished(pb: &Playback, _: &FakeHandle) -> bool {
        pb.state() == State::Finished
    }

    #[test]
    fn resume_plays_the_whole_file_then_finishes() {
        let (d, src) = ramp_dir("pb-all", 20_000);
        let (mut pb, fake, shared) = player(&d, 3_200);
        pb.apply(Cmd::Resume);
        assert_eq!(pb.state(), State::Playing);
        run_until(&mut pb, &fake, finished);
        assert_eq!(fake.played(), src);
        let snap = shared.snapshot();
        assert_eq!(snap.state, State::Finished);
        assert_eq!((snap.position_frames, snap.total_frames), (20_000, 20_000));
        assert_eq!(snap.rate, RATE);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn pause_discards_buffered_frames_and_resume_continues_without_a_gap_or_repeat() {
        let (d, src) = ramp_dir("pb-pause", 20_000);
        let (mut pb, fake, shared) = player(&d, 3_200);
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, |_, f| f.played().len() >= 5_000);
        pb.apply(Cmd::Pause);
        assert_eq!(pb.state(), State::Paused);
        let at_pause = fake.played().len() as u64;
        assert_eq!(shared.snapshot().position_frames, at_pause);
        assert_eq!(fake.buffered(), 0, "the buffer was dropped");
        // Time passing while paused plays nothing.
        fake.advance(5_000);
        pb.tick();
        assert_eq!(fake.played().len() as u64, at_pause);
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, finished);
        assert_eq!(fake.played(), src, "no gap, no repeat");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn seek_while_playing_and_while_paused() {
        let (d, src) = ramp_dir("pb-seek", 20_000);
        let (mut pb, fake, shared) = player(&d, 3_200);
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, |_, f| f.played().len() >= 2_000);
        let before = fake.played().len();
        pb.apply(Cmd::Seek(10_000));
        assert_eq!(pb.state(), State::Playing);
        run_until(&mut pb, &fake, finished);
        assert_eq!(&fake.played()[before..], &src[10_000..]);

        // Paused: the position moves at once and Resume plays from there.
        let (d2, src2) = ramp_dir("pb-seek2", 20_000);
        let (mut pb, fake, shared2) = player(&d2, 3_200);
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, |_, f| f.played().len() >= 1_000);
        pb.apply(Cmd::Pause);
        pb.apply(Cmd::Seek(5_000));
        assert_eq!(pb.state(), State::Paused);
        assert_eq!(shared2.snapshot().position_frames, 5_000);
        let before = fake.played().len();
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, finished);
        assert_eq!(&fake.played()[before..], &src2[5_000..]);
        let _ = shared;
        for d in [d, d2] {
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn seek_past_the_end_finishes_without_playing_more() {
        let (d, _src) = ramp_dir("pb-past", 20_000);
        let (mut pb, fake, shared) = player(&d, 3_200);
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, |_, f| f.played().len() >= 2_000);
        let before = fake.played().len();
        pb.apply(Cmd::Seek(u64::MAX));
        run_until(&mut pb, &fake, finished);
        assert_eq!(fake.played().len(), before);
        assert_eq!(shared.snapshot().position_frames, 20_000);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn resume_after_finished_or_at_the_end_restarts_from_zero() {
        let (d, src) = ramp_dir("pb-restart", 6_000);
        let (mut pb, fake, _) = player(&d, 3_200);
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, finished);
        pb.apply(Cmd::Resume);
        assert_eq!(pb.state(), State::Playing);
        run_until(&mut pb, &fake, |p, _| {
            p.state() == State::Finished && fake.played().len() == 12_000
        });
        let mut twice = src.clone();
        twice.extend(src.iter());
        assert_eq!(fake.played(), twice);

        // Paused at the very end (seek there, then press play): also from the start.
        let (d2, src2) = ramp_dir("pb-restart2", 6_000);
        let (mut pb, fake, _) = player(&d2, 3_200);
        pb.apply(Cmd::Seek(6_000));
        pb.apply(Cmd::Resume);
        run_until(&mut pb, &fake, finished);
        assert_eq!(fake.played(), src2);
        for d in [d, d2] {
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn position_is_what_was_played_not_what_was_buffered() {
        let (d, _src) = ramp_dir("pb-pos", 20_000);
        let (mut pb, fake, shared) = player(&d, 3_200);
        pb.apply(Cmd::Resume);
        pb.tick();
        assert!(fake.buffered() > 0);
        assert_eq!(
            shared.snapshot().position_frames,
            0,
            "nothing has been heard yet"
        );
        fake.advance(480);
        pb.tick();
        assert_eq!(shared.snapshot().position_frames, 480);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_device_error_fails_the_player_with_the_message() {
        let (d, _src) = ramp_dir("pb-fail", 20_000);
        let (mut pb, fake, shared) = player(&d, 3_200);
        fake.fail_writes("device lost");
        pb.apply(Cmd::Resume);
        assert_eq!(pb.state(), State::Failed);
        let snap = shared.snapshot();
        assert_eq!(snap.state, State::Failed);
        assert!(snap.error.unwrap().contains("device lost"));
        // Nothing panics afterwards.
        pb.tick();
        pb.apply(Cmd::Pause);
        pb.apply(Cmd::Seek(10));
        assert_eq!(pb.state(), State::Failed);
        let _ = fs::remove_dir_all(&d);
    }

    // ─── PlayerHandle on its own thread (still a fake sink) ──────────────────────────────────

    fn wait_for(h: &PlayerHandle, what: &str, cond: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let start = std::time::Instant::now();
        loop {
            let snap = h.snapshot();
            if cond(&snap) {
                return snap;
            }
            assert!(
                start.elapsed() < std::time::Duration::from_secs(20),
                "timed out waiting for {what}: {snap:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    fn fake_handle(d: &Path, per_wait: usize) -> PlayerHandle {
        PlayerHandle::start_with(d, move |_| {
            Ok(Box::new(FakeSink::new(3_200, per_wait).0) as Box<dyn Sink>)
        })
    }

    #[test]
    fn the_handle_plays_pauses_seeks_and_stops() {
        let (d, _src) = ramp_dir("pb-handle", 29_000);
        let h = fake_handle(&d, 160);
        assert_eq!(h.dir(), d.as_path());
        let snap = wait_for(&h, "playing", |s| {
            s.state == State::Playing && s.position_frames > 0
        });
        assert_eq!((snap.total_frames, snap.rate), (29_000, RATE));
        h.send(Cmd::Pause);
        let snap = wait_for(&h, "paused", |s| s.state == State::Paused);
        std::thread::sleep(std::time::Duration::from_millis(40));
        assert_eq!(
            h.snapshot().position_frames,
            snap.position_frames,
            "paused holds still"
        );
        h.send(Cmd::Seek(28_000));
        wait_for(&h, "seek while paused", |s| s.position_frames == 28_000);
        h.send(Cmd::Resume);
        let end = wait_for(&h, "finished", |s| s.state == State::Finished);
        assert_eq!(end.position_frames, 29_000);
        h.stop();
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_bad_folder_or_a_missing_device_fails_the_handle_with_an_error() {
        let empty = dir("pb-handle-empty");
        let h = fake_handle(&empty, 160);
        let snap = wait_for(&h, "failure", |s| s.state == State::Failed);
        assert!(snap.error.is_some());
        h.stop();

        let (d, _) = ramp_dir("pb-handle-nodev", 1_000);
        let h = PlayerHandle::start_with(&d, |_| Err("no output device".to_owned()));
        let snap = wait_for(&h, "failure", |s| s.state == State::Failed);
        assert_eq!(snap.error.as_deref(), Some("no output device"));
        drop(h);
        for d in [empty, d] {
            let _ = fs::remove_dir_all(&d);
        }
    }

    #[test]
    fn stop_closes_the_files_so_the_folder_can_be_deleted() {
        let (d, _) = ramp_dir("pb-handle-delete", 29_000);
        let h = fake_handle(&d, 16);
        wait_for(&h, "playing", |s| {
            s.state == State::Playing && s.position_frames > 0
        });
        h.stop();
        fs::remove_dir_all(&d).expect("no file is still open after stop()");
        assert!(!d.exists());
    }

    #[test]
    fn dropping_the_handle_also_closes_the_files() {
        let (d, _) = ramp_dir("pb-handle-drop", 29_000);
        let h = fake_handle(&d, 16);
        wait_for(&h, "playing", |s| s.position_frames > 0);
        drop(h);
        fs::remove_dir_all(&d).expect("no file is still open after drop");
    }

    /// Plays a tone through the real default output device while capturing that device in
    /// loopback, and checks the loopback heard it. Needs an output device and sound that is not
    /// muted: run with `cargo test plays_a_tone -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "needs a real output device"]
    fn plays_a_tone_and_the_loopback_hears_it() {
        use wasapi::{
            deinitialize, initialize_mta, DeviceEnumerator, Direction, SampleType, StreamMode,
            WaveFormat,
        };
        let d = dir("pb-tone");
        let tone: Vec<i16> = (0..48_000 * 4)
            .map(|i| {
                ((f64::from(i) * 2.0 * std::f64::consts::PI * 440.0 / 48_000.0).sin() * 16_000.0)
                    as i16
            })
            .collect();
        write_wav(&d.join("mic.wav"), 48_000, &tone);

        let _ = initialize_mta();
        let enumerator = DeviceEnumerator::new().unwrap();
        let mut client = enumerator
            .get_default_device(Direction::Render)
            .unwrap()
            .get_iaudioclient()
            .unwrap();
        let format = WaveFormat::new(32, 32, SampleType::Float, 48_000, 1, None);
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: 2_000_000,
        };
        client
            .initialize_client(&format, Direction::Capture, mode, None)
            .unwrap();
        let event = client.set_get_eventhandle().unwrap();
        let capture = client.get_audiocaptureclient().unwrap();
        client.start_stream().unwrap();

        // Amplitude of the 440 Hz tone in what the loopback captured for `millis` (Goertzel), so
        // other sound playing on the machine does not count as the tone.
        let tone_amplitude = |millis: u64| -> f64 {
            let start = std::time::Instant::now();
            let mut samples: Vec<f64> = Vec::new();
            let mut buf = Vec::new();
            while start.elapsed() < std::time::Duration::from_millis(millis) {
                let _ = event.wait_for_event(100);
                while let Ok(Some(frames)) = capture.get_next_packet_size() {
                    if frames == 0 {
                        break;
                    }
                    buf.resize(frames as usize * 4, 0);
                    let Ok((got, _)) = capture.read_from_device(&mut buf) else {
                        break;
                    };
                    for b in buf[..got as usize * 4].chunks_exact(4) {
                        samples.push(f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
                    }
                }
            }
            let w = 2.0 * std::f64::consts::PI * 440.0 / 48_000.0;
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (i, v) in samples.iter().enumerate() {
                re += v * (w * i as f64).cos();
                im += v * (w * i as f64).sin();
            }
            2.0 * (re * re + im * im).sqrt() / samples.len().max(1) as f64
        };

        let quiet = tone_amplitude(600);
        let h = PlayerHandle::start(&d);
        std::thread::sleep(std::time::Duration::from_millis(300));
        let heard = tone_amplitude(1_500);
        let snap = h.snapshot();
        h.stop();
        let _ = client.stop_stream();
        deinitialize();
        let _ = fs::remove_dir_all(&d);
        eprintln!("440 Hz amplitude: quiet {quiet:.5}, while playing {heard:.5}, {snap:?}");
        assert!(snap.error.is_none(), "{snap:?}");
        assert!(
            heard > 0.01 && heard > quiet * 5.0,
            "quiet {quiet}, heard {heard}"
        );
    }
}

/// Test helpers shared with the app's tests: a sink that plays into memory.
#[cfg(test)]
pub(crate) mod testing {
    use super::{PlayerHandle, Sink};
    use std::collections::VecDeque;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    pub(crate) struct FakeState {
        buffered: VecDeque<f32>,
        played: Vec<f32>,
        running: bool,
        capacity: usize,
        per_wait: usize,
        write_error: Option<String>,
    }

    /// The test's view of a `FakeSink`.
    #[derive(Clone)]
    pub(crate) struct FakeHandle(Arc<Mutex<FakeState>>);

    impl FakeHandle {
        fn lock(&self) -> std::sync::MutexGuard<'_, FakeState> {
            self.0.lock().unwrap_or_else(|e| e.into_inner())
        }

        /// Everything the "device" has played so far.
        pub(crate) fn played(&self) -> Vec<f32> {
            self.lock().played.clone()
        }

        pub(crate) fn buffered(&self) -> usize {
            self.lock().buffered.len()
        }

        /// The device consumes up to `n` buffered frames, if the stream is running.
        pub(crate) fn advance(&self, n: usize) {
            let mut st = self.lock();
            if !st.running {
                return;
            }
            let n = n.min(st.buffered.len());
            let taken: Vec<f32> = st.buffered.drain(..n).collect();
            st.played.extend(taken);
        }

        pub(crate) fn fail_writes(&self, message: &str) {
            self.lock().write_error = Some(message.to_owned());
        }
    }

    /// A sink with room for `capacity` frames that plays `per_wait` frames on every `wait`.
    pub(crate) struct FakeSink(FakeHandle);

    impl FakeSink {
        pub(crate) fn new(capacity: usize, per_wait: usize) -> (FakeSink, FakeHandle) {
            let h = FakeHandle(Arc::new(Mutex::new(FakeState {
                buffered: VecDeque::new(),
                played: Vec::new(),
                running: false,
                capacity,
                per_wait,
                write_error: None,
            })));
            (FakeSink(h.clone()), h)
        }
    }

    impl Sink for FakeSink {
        fn start(&mut self) -> Result<(), String> {
            self.0.lock().running = true;
            Ok(())
        }

        fn stop(&mut self) -> Result<(), String> {
            self.0.lock().running = false;
            Ok(())
        }

        fn discard(&mut self) -> Result<usize, String> {
            let mut st = self.0.lock();
            let n = st.buffered.len();
            st.buffered.clear();
            Ok(n)
        }

        fn space(&mut self) -> Result<usize, String> {
            let st = self.0.lock();
            Ok(st.capacity - st.buffered.len())
        }

        fn padding(&mut self) -> Result<usize, String> {
            Ok(self.0.lock().buffered.len())
        }

        fn write(&mut self, block: &[f32]) -> Result<(), String> {
            let mut st = self.0.lock();
            if let Some(e) = &st.write_error {
                return Err(e.clone());
            }
            st.buffered.extend(block.iter().copied());
            Ok(())
        }

        fn wait(&mut self) {
            std::thread::sleep(std::time::Duration::from_millis(1));
            let per = self.0.lock().per_wait;
            self.0.advance(per);
        }
    }

    /// A player on `dir` whose "device" runs at about real time (16 frames per millisecond is
    /// real time at 16 kHz); for the app's tests.
    pub(crate) fn start_fake(dir: &Path) -> PlayerHandle {
        PlayerHandle::start_with(dir, |rate| {
            let per_wait = (rate as usize / 1000).max(1);
            Ok(Box::new(FakeSink::new(rate as usize / 5, per_wait).0) as Box<dyn Sink>)
        })
    }
}
