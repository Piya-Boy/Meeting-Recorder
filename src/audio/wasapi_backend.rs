//! WASAPI capture (Windows): microphone, whole-system loopback, per-process loopback.
//!
//! Each stream runs on its own thread (COM objects are created there), in shared event mode
//! with automatic format conversion to mono 32-bit float at the session rate (stereo as a
//! second try). Chunks carry their position on the session timeline, computed from the
//! device's QPC timestamps, so gaps (loopback sends nothing while no sound plays) and clock
//! drift show up as differences the worker's `TrackClock` corrects.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use wasapi::{
    deinitialize, initialize_mta, AudioCaptureClient, AudioClient, DeviceEnumerator, Direction,
    Handle, SampleType, SessionState, StreamMode, WaveFormat,
};
use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
};

use super::backend::{Backend, Chunk, Msg, SourceKind, Stream};
use super::procfind;
use super::session::{DeviceInfo, Source};

/// Buffer duration asked from the audio engine (20 ms, in 100 ns units).
const BUFFER_HNS: i64 = 200_000;
/// How long a capture thread waits for the device event before checking the stop flag.
const WAIT_MS: u32 = 50;
/// How often a process source checks that its process still exists.
const ALIVE_CHECK: Duration = Duration::from_secs(1);
const START_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SampleFormat {
    F32,
    // Capture asks Windows to convert to f32 (autoconvert), so only the tests build these two;
    // the converter stays for a device that refuses the conversion.
    #[allow(dead_code)]
    I16,
    #[allow(dead_code)]
    I32,
}

/// Interleaved `data` with `channels` channels to mono i16: channels are averaged, floats
/// clamped to [-1, 1].
pub(crate) fn to_mono_i16(data: &[u8], channels: u16, format: SampleFormat) -> Vec<i16> {
    let channels = usize::from(channels.max(1));
    let width = match format {
        SampleFormat::I16 => 2,
        SampleFormat::F32 | SampleFormat::I32 => 4,
    };
    let sample = |b: &[u8]| -> f32 {
        match format {
            SampleFormat::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]).clamp(-1.0, 1.0),
            SampleFormat::I16 => f32::from(i16::from_le_bytes([b[0], b[1]])) / 32_767.0,
            SampleFormat::I32 => {
                f32::from((i32::from_le_bytes([b[0], b[1], b[2], b[3]]) >> 16) as i16) / 32_767.0
            }
        }
    };
    data.chunks_exact(width * channels)
        .map(|frame| {
            let sum: f32 = frame.chunks_exact(width).map(sample).sum();
            ((sum / channels as f32).clamp(-1.0, 1.0) * 32_767.0).round() as i16
        })
        .collect()
}

/// Frames at `rate` between `origin` and `now` (0 if `now` is earlier).
pub(crate) fn frames_since(origin: Instant, now: Instant, rate: u32) -> u64 {
    (now.saturating_duration_since(origin).as_nanos() * u128::from(rate) / 1_000_000_000) as u64
}

/// Places a stream's chunks on the session timeline. The first chunk is anchored to the wall
/// clock; later ones follow the device timestamps (100 ns units), which also expose gaps. A
/// chunk without a usable timestamp continues after the previous one.
pub(crate) struct Timeline {
    origin: Instant,
    rate: u32,
    anchor: Option<(u64, u64)>, // (device timestamp, frame)
    next_frame: u64,
    started: bool,
}

impl Timeline {
    pub(crate) fn new(origin: Instant, rate: u32) -> Self {
        Self {
            origin,
            rate,
            anchor: None,
            next_frame: 0,
            started: false,
        }
    }

    pub(crate) fn start_frame(
        &mut self,
        ts: u64,
        ts_valid: bool,
        frames: u64,
        now: Instant,
    ) -> u64 {
        let frame = match self.anchor {
            Some((ts0, f0)) if ts_valid && ts >= ts0 => {
                f0 + (ts - ts0) * u64::from(self.rate) / 10_000_000
            }
            Some(_) => self.next_frame,
            None if self.started && !ts_valid => self.next_frame,
            None => {
                let f = frames_since(self.origin, now, self.rate).saturating_sub(frames);
                if ts_valid {
                    self.anchor = Some((ts, f));
                }
                f
            }
        };
        self.started = true;
        self.next_frame = frame + frames;
        frame
    }
}

pub struct WasapiBackend;

impl WasapiBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasapiBackend {
    fn default() -> Self {
        Self::new()
    }
}

struct WasapiStream {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Stream for WasapiStream {
    fn stop(self: Box<Self>) {} // Drop does the work
}

impl Drop for WasapiStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Backend for WasapiBackend {
    fn open(
        &self,
        kind: SourceKind,
        origin: Instant,
        sample_rate: u32,
        tx: Sender<Msg>,
    ) -> Result<Box<dyn Stream>, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel();
        let thread_stop = stop.clone();
        let join = std::thread::Builder::new()
            .name("wasapi-capture".into())
            .spawn(move || capture_thread(kind, origin, sample_rate, tx, thread_stop, ready_tx))
            .map_err(|e| e.to_string())?;
        let stream = WasapiStream {
            stop,
            join: Some(join),
        };
        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(Box::new(stream)),
            Ok(Err(e)) => Err(e), // dropping `stream` joins the finished thread
            Err(_) => Err("Starting audio capture timed out".to_string()),
        }
    }

    fn meeting_target(&self) -> Option<u32> {
        let _ = initialize_mta();
        procfind::pick_target(&procfind::list_processes(), &active_audio_pids())
    }

    fn list_devices(&self) -> (Vec<DeviceInfo>, Vec<DeviceInfo>) {
        let _ = initialize_mta();
        (devices(Direction::Render), devices(Direction::Capture))
    }
}

fn devices(direction: Direction) -> Vec<DeviceInfo> {
    let Ok(enumerator) = DeviceEnumerator::new() else {
        return Vec::new();
    };
    let default_id = enumerator
        .get_default_device(direction)
        .ok()
        .and_then(|d| d.get_id().ok());
    let Ok(collection) = enumerator.get_device_collection(direction) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for device in &collection {
        let Ok(device) = device else { continue };
        let Ok(id) = device.get_id() else { continue };
        let name = device.get_friendlyname().unwrap_or_else(|_| id.clone());
        let is_default = default_id.as_deref() == Some(id.as_str());
        out.push(DeviceInfo {
            id,
            name,
            is_default,
        });
    }
    out
}

/// Pids that own an active audio session on any output device.
fn active_audio_pids() -> HashSet<u32> {
    let mut out = HashSet::new();
    let Ok(enumerator) = DeviceEnumerator::new() else {
        return out;
    };
    let Ok(collection) = enumerator.get_device_collection(Direction::Render) else {
        return out;
    };
    for device in &collection {
        let Ok(device) = device else { continue };
        let Ok(manager) = device.get_iaudiosessionmanager() else {
            continue;
        };
        let Ok(sessions) = manager.get_audiosessionenumerator() else {
            continue;
        };
        let Ok(count) = sessions.get_count() else {
            continue;
        };
        for i in 0..count {
            let Ok(session) = sessions.get_session(i) else {
                continue;
            };
            if matches!(session.get_state(), Ok(SessionState::Active)) {
                if let Ok(pid) = session.get_process_id() {
                    out.insert(pid);
                }
            }
        }
    }
    out
}

fn process_alive(pid: u32) -> bool {
    // SAFETY: the handle is checked for failure and closed before returning.
    unsafe {
        let h = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if h == 0 {
            return false;
        }
        let waited = WaitForSingleObject(h, 0);
        CloseHandle(h);
        waited != WAIT_OBJECT_0
    }
}

struct Opened {
    client: AudioClient,
    event: Handle,
    capture: AudioCaptureClient,
    channels: usize,
}

fn make_client(kind: &SourceKind, enumerator: &DeviceEnumerator) -> Result<AudioClient, String> {
    let pick = |id: &Option<String>, dir: Direction| {
        match id {
            Some(id) => enumerator.get_device(id),
            None => enumerator.get_default_device(dir),
        }
        .and_then(|d| d.get_iaudioclient())
        .map_err(|e| e.to_string())
    };
    match kind {
        SourceKind::Mic { device } => pick(device, Direction::Capture),
        // A render device captured as `Capture` is a loopback.
        SourceKind::WholeSystem { device } => pick(device, Direction::Render),
        SourceKind::Process { pid } => {
            AudioClient::new_application_loopback_client(*pid, true).map_err(|e| e.to_string())
        }
    }
}

/// Opens the stream, trying mono then stereo float. A failed `Initialize` leaves a client
/// unusable, so every attempt makes a fresh one.
fn open_stream(kind: &SourceKind, rate: u32) -> Result<Opened, String> {
    let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let mut last = String::new();
    for channels in [1usize, 2] {
        let mut client = make_client(kind, &enumerator)?;
        let format = WaveFormat::new(32, 32, SampleType::Float, rate as usize, channels, None);
        let mode = StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: BUFFER_HNS,
        };
        if let Err(e) = client.initialize_client(&format, Direction::Capture, mode, None) {
            last = e.to_string();
            continue;
        }
        let event = client.set_get_eventhandle().map_err(|e| e.to_string())?;
        let capture = client.get_audiocaptureclient().map_err(|e| e.to_string())?;
        client.start_stream().map_err(|e| e.to_string())?;
        return Ok(Opened {
            client,
            event,
            capture,
            channels,
        });
    }
    Err(last)
}

fn capture_thread(
    kind: SourceKind,
    origin: Instant,
    rate: u32,
    tx: Sender<Msg>,
    stop: Arc<AtomicBool>,
    ready: Sender<Result<(), String>>,
) {
    let _ = initialize_mta();
    match open_stream(&kind, rate) {
        Err(e) => {
            let _ = ready.send(Err(e));
        }
        Ok(opened) => {
            let _ = ready.send(Ok(()));
            run_capture(&kind, origin, rate, &tx, &stop, &opened);
            let _ = opened.client.stop_stream();
        }
    }
    deinitialize();
}

/// Reads packets until stopped; reports `Lost` once if the device or process goes away.
fn run_capture(
    kind: &SourceKind,
    origin: Instant,
    rate: u32,
    tx: &Sender<Msg>,
    stop: &AtomicBool,
    o: &Opened,
) {
    let source: Source = kind.source();
    let frame_bytes = o.channels * 4;
    let mut timeline = Timeline::new(origin, rate);
    let mut buf = Vec::new();
    let mut last_alive = Instant::now();
    let lost = || {
        let _ = tx.send(Msg::Lost(source));
    };
    while !stop.load(Ordering::Relaxed) {
        let _ = o.event.wait_for_event(WAIT_MS); // a timeout just means no packet yet
        loop {
            let frames = match o.capture.get_next_packet_size() {
                Ok(Some(n)) => n as usize,
                Ok(None) => 0,
                Err(_) => return lost(),
            };
            if frames == 0 {
                break;
            }
            buf.resize(frames * frame_bytes, 0);
            let (n, info) = match o.capture.read_from_device(&mut buf) {
                Ok(v) => v,
                Err(_) => return lost(),
            };
            if n == 0 {
                break;
            }
            let samples = to_mono_i16(
                &buf[..n as usize * frame_bytes],
                o.channels as u16,
                SampleFormat::F32,
            );
            let valid = !info.flags.timestamp_error && info.timestamp != 0;
            let start =
                timeline.start_frame(info.timestamp, valid, samples.len() as u64, Instant::now());
            if tx
                .send(Msg::Chunk(Chunk {
                    source,
                    start_frame: start,
                    samples,
                }))
                .is_err()
            {
                return;
            }
        }
        if let SourceKind::Process { pid } = kind {
            if last_alive.elapsed() >= ALIVE_CHECK {
                last_alive = Instant::now();
                if !process_alive(*pid) {
                    return lost();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn to_mono_i16_f32_stereo_averages() {
        let data: Vec<u8> = [0.5f32, -0.5, 1.0, 1.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let out = to_mono_i16(&data, 2, SampleFormat::F32);
        assert_eq!(out, vec![0, i16::MAX]);
    }

    #[test]
    fn to_mono_i16_clamps_overrange() {
        let data: Vec<u8> = [2.0f32, 2.0, -3.0, -3.0]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let out = to_mono_i16(&data, 2, SampleFormat::F32);
        assert_eq!(out, vec![i16::MAX, -i16::MAX]);
    }

    #[test]
    fn to_mono_i16_i16_mono_passthrough() {
        let data: Vec<u8> = [100i16, -200, 300]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            to_mono_i16(&data, 1, SampleFormat::I16),
            vec![100, -200, 300]
        );
    }

    #[test]
    fn to_mono_i16_i32_takes_high_half() {
        let data: Vec<u8> = [0x1234_0000i32, -0x1000_0000]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        assert_eq!(
            to_mono_i16(&data, 1, SampleFormat::I32),
            vec![0x1234, -0x1000]
        );
    }

    #[test]
    fn frames_since_matches_rate() {
        let origin = Instant::now();
        assert_eq!(
            frames_since(origin, origin + Duration::from_millis(500), 48_000),
            24_000
        );
        assert_eq!(
            frames_since(origin + Duration::from_secs(1), origin, 48_000),
            0
        );
    }

    fn ten_ms(rate: u32) -> u64 {
        u64::from(rate) / 100
    }

    #[test]
    fn timeline_first_chunk_uses_wall_clock() {
        let origin = Instant::now();
        let mut t = Timeline::new(origin, 48_000);
        let start = t.start_frame(5_000_000, true, 480, origin + Duration::from_secs(1));
        assert_eq!(start, 48_000 - 480);
    }

    #[test]
    fn timeline_later_chunks_follow_device_timestamps() {
        let origin = Instant::now();
        let now = origin + Duration::from_secs(1);
        let mut t = Timeline::new(origin, 48_000);
        let first = t.start_frame(5_000_000, true, 480, now);
        assert_eq!(
            t.start_frame(5_100_000, true, 480, now),
            first + ten_ms(48_000)
        );
        // One second with no packets (loopback while nothing plays).
        assert_eq!(
            t.start_frame(15_100_000, true, 480, now),
            first + ten_ms(48_000) + 48_000
        );
    }

    #[test]
    fn timeline_invalid_timestamp_continues_after_previous_chunk() {
        let origin = Instant::now();
        let now = origin + Duration::from_secs(1);
        let mut t = Timeline::new(origin, 48_000);
        let first = t.start_frame(5_000_000, true, 480, now);
        assert_eq!(t.start_frame(0, false, 480, now), first + 480);
        assert_eq!(t.start_frame(0, false, 480, now), first + 960);
    }

    #[test]
    fn timeline_backwards_timestamp_continues_after_previous_chunk() {
        let origin = Instant::now();
        let now = origin + Duration::from_secs(1);
        let mut t = Timeline::new(origin, 48_000);
        let first = t.start_frame(5_000_000, true, 480, now);
        assert_eq!(t.start_frame(4_000_000, true, 480, now), first + 480);
    }

    // Hardware tests: run by hand with `cargo test -- --ignored wasapi_`.

    fn collect(kind: SourceKind, secs: u64) -> (Vec<Msg>, Result<(), String>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let backend = WasapiBackend::new();
        let stream = match backend.open(kind, Instant::now(), 48_000, tx) {
            Ok(s) => s,
            Err(e) => return (Vec::new(), Err(e)),
        };
        std::thread::sleep(Duration::from_secs(secs));
        stream.stop();
        (rx.try_iter().collect(), Ok(()))
    }

    #[test]
    #[ignore = "needs audio devices"]
    fn wasapi_lists_devices() {
        let (system, mic) = WasapiBackend::new().list_devices();
        assert!(
            !system.is_empty() && !mic.is_empty(),
            "system {system:?} mic {mic:?}"
        );
        assert_eq!(system.iter().filter(|d| d.is_default).count(), 1);
        assert_eq!(mic.iter().filter(|d| d.is_default).count(), 1);
    }

    #[test]
    #[ignore = "needs a microphone"]
    fn wasapi_mic_captures_two_seconds() {
        let (msgs, opened) = collect(SourceKind::Mic { device: None }, 2);
        opened.expect("open default mic");
        let frames: usize = msgs
            .iter()
            .map(|m| match m {
                Msg::Chunk(c) => c.samples.len(),
                Msg::Lost(_) => 0,
            })
            .sum();
        assert!((76_800..=115_200).contains(&frames), "got {frames} frames");
    }

    #[test]
    #[ignore = "needs an output device"]
    fn wasapi_whole_system_loopback_opens() {
        let (msgs, opened) = collect(SourceKind::WholeSystem { device: None }, 1);
        opened.expect("open default loopback");
        assert!(!msgs.iter().any(|m| matches!(m, Msg::Lost(_))));
    }

    #[test]
    #[ignore = "needs Windows 11 / 10 build with process loopback"]
    fn wasapi_process_loopback_opens_for_own_pid() {
        let (msgs, opened) = collect(
            SourceKind::Process {
                pid: std::process::id(),
            },
            1,
        );
        opened.expect("open process loopback");
        assert!(!msgs.iter().any(|m| matches!(m, Msg::Lost(_))));
    }
}
