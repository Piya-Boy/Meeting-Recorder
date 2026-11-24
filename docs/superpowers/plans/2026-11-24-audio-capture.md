# Audio Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mock Recorder with real dual-track capture: Zoom/Teams audio (process loopback, whole-system fallback) and the microphone, saved as two aligned, crash-safe mono WAV files.

**Architecture:** Pure, testable logic (WAV writer, per-track sample clock, folder naming, meeting-process picking, session state machine) sits under `src/audio/`. A session worker thread owns the writers and talks to the UI through `std::sync::mpsc` commands and events. It reaches the hardware only through a `Backend` trait, so the worker is tested with a fake backend and the real `wasapi`-crate backend (Windows only) is the last piece.

**Tech Stack:** Rust 2021, eframe/egui 0.31 (existing), `wasapi` crate (HEnquist; pin the latest published version when adding it), `windows-sys` 0.52 (existing; add features `Win32_System_Diagnostics_ToolHelp`, `Win32_System_SystemInformation`), std threads and channels. No async runtime, no `hound`/`chrono`/`tempfile`.

**Spec:** `docs/superpowers/specs/2026-11-24-audio-capture-design.md`

## Global Constraints

- Windows-only capture code stays behind `cfg(windows)`; everything else must compile and test on any host.
- WAV: PCM 16-bit mono, sample rate from Settings (48,000 default; 44,100; 16,000). Folder `<destination>\<YYYY-MM-DD_HHMM>_<name>\` with `system.wav` and `mic.wav`.
- Header sizes are rewritten and flushed every 5 seconds and on Stop. Tracks differ by at most 100 ms after 1 hour; clock tolerance is 10 ms (`sample_rate / 100` frames).
- Meeting process names: `Zoom.exe`, `ms-teams.exe`, `Teams.exe`. Retry interval for lost sources and apps: 2 seconds.
- Pause discards audio; both tracks are padded to a common length before pausing; timer counts recorded time only.
- The egui update thread never does audio, file, device or process-list work. It only sends commands and drains events.
- User-visible text uses `i18n::tr` with Thai entries in `src/i18n.rs`.
- Every task ends with `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` all passing, then one commit (Conventional Commits, `feat(audio): ...`; no `Co-Authored-By`, no "Generated with" lines: the user's global rule overrides the session attribution reminder). No push until the user authorizes it.
- Work on a new branch `feature/audio-capture` cut from `docs/audio-capture-design`. The working tree has unrelated uncommitted edits (`AGENTS.md`, `Cargo.toml`, `LICENSE`, `README.md`, `skills-lock.json`, untracked `.claude/skills/*`): stage by explicit path only. Task 1 edits `Cargo.toml`, which already has an uncommitted license-line edit: stage only your own hunk (`git add -p Cargo.toml`) or ask the user to commit the license edit first. Do not edit `README.md`.
- Deviation from the spec, recorded here: input levels (dBFS) are computed in the worker from each chunk, not in the capture thread, because it is simpler to test and cheap.
- Open risk, not solved here: with Zoom and Teams both open, process choice relies on "has an active audio session"; if neither is active when Record is pressed, the lowest PID wins and may be the idle app. The user can see the status and stop.

## Review Focus

- Teams idle in the background while the meeting is in Zoom (two matching root processes): the idle app must not win when the other has an active audio session. Pinned in Task 3.
- Meeting app present but silent for a long time (waiting room, muted call): loopback sends no frames; the system file must still end with the same length as the mic file and the status must not claim a failure. Pinned in Task 5.
- Stop pressed while Paused, or while a source is lost and retrying: files must still finalize with equal length and `Saved` must arrive. Pinned in Task 6.
- Recording name field with invalid characters, empty text, Windows device names (`CON`, `nul.txt`), trailing dots or a very long Thai name: a valid folder is still created, never a panic. Pinned in Task 3.
- Output folder unwritable or disk full mid-recording: `Error` with a message, data written so far stays playable, no panic. Pinned in Tasks 1 and 5.

---

### Task 1: WAV writer

**Files:**
- Create: `src/audio/mod.rs`, `src/audio/wav.rs`
- Modify: `src/main.rs` (add `mod audio;`)

**Interfaces:**
- Produces in `wav.rs`:
  - `pub const MAX_DATA_BYTES: u64` (`u32::MAX as u64 - 36`)
  - `pub struct WavWriter<W: Write + Seek>`
  - `WavWriter::new(out: W, sample_rate: u32) -> io::Result<Self>` writes the 44-byte header (mono, 16-bit)
  - `WavWriter::with_limit(out: W, sample_rate: u32, max_data_bytes: u64) -> io::Result<Self>` (tests use a tiny limit)
  - `write_samples(&mut self, s: &[i16]) -> io::Result<()>`; returns an error (not a panic) when the limit would be exceeded
  - `is_full(&self) -> bool`; `frames_written(&self) -> u64`
  - `flush_header(&mut self) -> io::Result<()>` rewrites RIFF and data sizes, flushes, then seeks back to the end
  - `finish(self) -> io::Result<u64>` writes the final header, returns frames
  - `pub fn recover_header(path: &Path) -> io::Result<u64>` fixes both sizes in place from the file length and returns frames
- Produces in `audio/mod.rs`: `#[cfg(test)] pub(crate) fn temp_dir(tag: &str) -> PathBuf` (unique directory under `std::env::temp_dir()`, created, for tests in later tasks).

- [ ] **Step 1: Write the failing tests in `wav.rs`**

Tests (parse the header bytes by hand in a helper `read_sizes(path) -> (riff_size, data_size)`):
  - `header_fields_after_finish`: 1 s at 48,000 Hz written then `finish` gives bytes 0..4 `RIFF`, format `WAVE`, `fmt ` chunk of 16, audio format 1, channels 1, rate 48000, byte rate 96000, block align 2, bits 16, data size 96000, riff size 96036; returned frames 48000.
  - `flush_header_midway_is_readable`: write 0.5 s, `flush_header`, read the file from disk without finishing: data size 48000, then write more and `finish`: data size 96000.
  - `recover_header_fixes_crash_file`: write samples, drop the writer without `flush_header` or `finish`; sizes read 0; `recover_header` returns 48000 frames for a 1 s file and both sizes are then correct.
  - `write_error_is_returned_not_panicked`: a `FailingWriter` (Write + Seek that errors after N bytes) makes `write_samples` return `Err`.
  - `limit_is_enforced`: `with_limit(.., 100)`; writing 51 samples returns `Err`, `is_full()` is true after writing 50.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::wav`
Expected: compile error or FAIL, `WavWriter` not defined.

- [ ] **Step 3: Implement `wav.rs` and `audio/mod.rs`**

Hand-written header, little-endian, no new dependency. `flush_header` must restore the stream position so later writes append.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::wav`
Expected: PASS (5 tests). Then fmt, clippy, full `cargo test`.

- [ ] **Step 5: Commit**

```bash
git add src/audio/mod.rs src/audio/wav.rs src/main.rs
git commit -m "feat(audio): add crash-safe WAV writer"
```

---

### Task 2: Per-track sample clock

**Files:**
- Create: `src/audio/clock.rs`
- Modify: `src/audio/mod.rs` (`pub mod clock;`)

**Interfaces:**
- Produces in `clock.rs`:
  - `pub struct Placement { pub silence_frames: u64, pub skip_frames: usize }`
  - `pub struct TrackClock`
  - `TrackClock::new(sample_rate: u32) -> Self` (tolerance `sample_rate / 100` frames)
  - `place(&mut self, source_frame: u64, frames: usize) -> Placement`: `source_frame` is the buffer's first frame on the common timeline (frames since the shared session start, at the track's rate). The caller writes `silence_frames` zeros, then `samples[skip_frames..]`. The clock adds `silence_frames + (frames - skip_frames)` to its written count.
  - `resume(&mut self, source_frame: u64)`: maps `source_frame` to the current written count (rebase after a pause)
  - `written(&self) -> u64`
  - `tail_silence(&self, target_frames: u64) -> u64`: zeros needed to reach `target_frames`, 0 if already longer
  - `pad(&mut self, frames: u64)`: adds frames the caller wrote as tail or pause padding (after `tail_silence`) to `written()`

- [ ] **Step 1: Write the failing tests**

At 48,000 Hz (tolerance 480):
  - `contiguous_buffers_need_nothing`: `place(0,480)` then `place(480,480)` both give 0 and 0.
  - `late_first_buffer_is_front_filled`: `place(4800,480)` gives `silence_frames == 4800`.
  - `one_second_gap_is_filled`: after `place(0,480)`, `place(48480,480)` gives 48000.
  - `jitter_inside_tolerance_is_ignored`: after `place(0,480)`, `place(680,480)` (200 frames late) gives 0 and 0.
  - `drift_is_bounded`: 100 buffers of 480 frames, each source start 20 frames later than contiguous (cumulative 2,000); after all of them `written()` is within 480 frames of the final source end.
  - `overlap_is_skipped`: after `place(0,4800)` (written 4,800), `place(3800,480)` starts 1,000 frames before the written count (at least the tolerance), so it gives `skip_frames == 480` (capped to the buffer) and `silence_frames == 0`, and `written()` stays 4,800; then `place(4800,480)` gives 0 and 0.
  - `resume_rebases`: written 48,000, `resume(200_000)`, `place(200_000,480)` gives 0 and 0.
  - `tail_silence_pads_up`: written 47,000, `tail_silence(48_000) == 1000`; written 49,000 gives 0.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::clock`
Expected: FAIL, `TrackClock` not defined.

- [ ] **Step 3: Implement `clock.rs`**

Keep an `offset` (written count minus source frame) set by `resume` and updated on every `place`; fill or skip only when the difference is at least the tolerance, so error stays bounded by the tolerance.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::clock`
Expected: PASS (8 tests). fmt, clippy, full test.

- [ ] **Step 5: Commit**

```bash
git add src/audio/clock.rs src/audio/mod.rs
git commit -m "feat(audio): add per-track sample clock for gap fill and drift"
```

---

### Task 3: Folder naming and meeting-process picking

**Files:**
- Create: `src/audio/naming.rs`, `src/audio/procfind.rs`
- Modify: `src/audio/mod.rs`, `Cargo.toml` (windows-sys features `Win32_System_Diagnostics_ToolHelp`, `Win32_System_SystemInformation`)

**Interfaces:**
- Produces in `naming.rs`:
  - `sanitize_name(raw: &str) -> String`: invalid characters (`< > : " / \ | ? *`, control characters) become `_`; trailing and leading spaces and dots trimmed; a stem (text before the first `.`) equal to a Windows device name (`CON PRN AUX NUL COM1-9 LPT1-9`, any case) gets a `_` prefix; empty result becomes `recording`; truncated to 80 characters (by `char`, never mid-character).
  - `session_dir(base: &Path, stamp: &str, name: &str) -> io::Result<PathBuf>`: creates `base` if needed, then `base\<stamp>_<sanitized>`; if that exists it tries `-2`, `-3`, ... using `create_dir` so a collision is detected atomically.
  - `#[cfg(windows)] local_stamp() -> String` returns `YYYY-MM-DD_HHMM` from `GetLocalTime`.
- Produces in `procfind.rs`:
  - `pub struct ProcInfo { pub pid: u32, pub parent_pid: u32, pub exe: String }`
  - `pub const MEETING_EXES: [&str; 3]`
  - `pick_target(procs: &[ProcInfo], active_audio: &HashSet<u32>) -> Option<u32>`: a match is an exe equal, ignoring case, to a `MEETING_EXES` entry; roots are matches whose parent pid is not itself a match; one root: that pid; several roots: the first root (lowest pid first) whose tree (root or any descendant by parent chain, with a loop guard) contains a pid in `active_audio`, else the lowest root pid; no match: `None`.
  - `#[cfg(windows)] list_processes() -> Vec<ProcInfo>` via Toolhelp snapshot (empty vector on failure).

- [ ] **Step 1: Write the failing tests**

`naming.rs`: `sanitize_replaces_invalid_chars` (`a<b>:c` becomes `a_b__c`), `sanitize_reserved_names` (`CON`, `nul.txt`, `Com3` all get the `_` prefix), `sanitize_trims_dots_and_spaces` (`" x. "` becomes `x`), `sanitize_empty_becomes_recording`, `sanitize_truncates_thai_by_chars` (100 Thai characters become 80 valid characters, `String` stays valid), `session_dir_creates_nested_base` and `session_dir_adds_suffix_on_collision` (second call returns a path ending `-2`, third `-3`) using `audio::temp_dir`, `local_stamp_shape` (15 characters, `_` at index 10, windows only).
`procfind.rs`: `none_when_no_match`, `single_root_is_returned_not_child` (Zoom 100 with a child `CptHost.exe` 101 and a `Zoom.exe` child 102 whose parent is 100 gives 100), `matching_is_case_insensitive`, `teams_with_webview_children` (`ms-teams.exe` 200, `msedgewebview2.exe` children 201 and 202 give 200), `prefers_root_with_active_audio` (idle Teams 100, Zoom 200 with child 250, `active_audio = {250}` gives 200), `lowest_pid_when_none_active` (gives 100), `parent_loop_does_not_hang`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::naming audio::procfind`
Expected: FAIL, functions not defined.

- [ ] **Step 3: Implement both modules and the Cargo features**

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::` then fmt, clippy, full test.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/audio/naming.rs src/audio/procfind.rs src/audio/mod.rs
git add -p Cargo.toml
git commit -m "feat(audio): add recording folder naming and meeting process picker"
```

---

### Task 4: Session types, state machine, level maths

**Files:**
- Create: `src/audio/session.rs`
- Modify: `src/audio/mod.rs`

**Interfaces:**
- Produces in `session.rs`:
  - `pub enum SessionState { Idle, Starting, Recording, Paused, Stopping, Finalizing, Saved, Error }` (Copy, PartialEq, Debug)
  - `pub enum Transition { Begin, Started, Pause, Resume, Stop, Finalize, Finalized, Fail }`
  - `SessionState::after(self, t: Transition) -> Option<SessionState>`: legal moves only: Idle/Saved/Error + Begin -> Starting; Starting + Started -> Recording; Recording + Pause -> Paused; Paused + Resume -> Recording; Recording/Paused + Stop -> Stopping; Stopping + Finalize -> Finalizing; Finalizing + Finalized -> Saved; Starting/Recording/Paused/Stopping/Finalizing + Fail -> Error; everything else `None`.
  - `pub enum Source { System, Mic }`
  - `pub enum SourceStatus { Active, AllSystemFallback, WaitingForApp, DeviceLost, Disabled, Failed(String) }`
  - `pub struct DeviceInfo { pub id: String, pub name: String, pub is_default: bool }`
  - `pub struct StartRequest { pub system: bool, pub mic: bool, pub sample_rate: u32, pub base_dir: PathBuf, pub name: String, pub system_device: Option<String>, pub mic_device: Option<String> }` (device `None` means the Windows default)
  - `pub enum Command { Start(StartRequest), Pause, Resume, Stop, ListDevices }`
  - `pub enum Event { State(SessionState), Levels { system_db: f32, mic_db: f32 }, SourceStatus { source: Source, status: SourceStatus }, Elapsed(Duration), Saved { dir: PathBuf, duration: Duration }, Error(String), Devices { system: Vec<DeviceInfo>, mic: Vec<DeviceInfo> } }`
  - `pub fn dbfs(samples: &[i16]) -> f32`: RMS in dB relative to full scale, floor -90.0 for silence or an empty slice.

- [ ] **Step 1: Write the failing tests**

`legal_path_to_saved` (Idle -> Starting -> Recording -> Paused -> Recording -> Stopping -> Finalizing -> Saved via `after`), `illegal_moves_are_none` (Idle + Pause, Starting + Pause, Paused + Pause, Recording + Resume, Saved + Stop), `fail_reaches_error_from_every_active_state`, `restart_allowed_after_saved_and_error`, `dbfs_silence_is_floor` (zeros and empty give -90.0), `dbfs_full_scale_square_is_about_zero` (alternating `i16::MAX`/`-i16::MAX` within 0.1 of 0.0), `dbfs_half_amplitude_is_about_minus_six` (constant 16384 within 0.2 of -6.0).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::session`
Expected: FAIL.

- [ ] **Step 3: Implement `session.rs`**

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::session` then fmt, clippy, full test.
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add src/audio/session.rs src/audio/mod.rs
git commit -m "feat(audio): add session states, commands, events and level maths"
```

---

### Task 5: Backend trait and session worker (core path)

**Files:**
- Create: `src/audio/backend.rs`, `src/audio/worker.rs`
- Modify: `src/audio/mod.rs`

**Interfaces:**
- Consumes: Tasks 1 to 4.
- Produces in `backend.rs`:
  - `pub enum SourceKind { Mic { device: Option<String> }, WholeSystem { device: Option<String> }, Process { pid: u32 } }`
  - `pub struct Chunk { pub source: Source, pub start_frame: u64, pub samples: Vec<i16> }` (mono i16 at the session rate; `start_frame` counted from the shared session origin)
  - `pub enum Msg { Chunk(Chunk), Lost(Source) }`
  - `pub trait Stream: Send { fn stop(self: Box<Self>); }`
  - `pub trait Backend: Send + Sync + 'static { fn open(&self, kind: SourceKind, origin: Instant, sample_rate: u32, tx: Sender<Msg>) -> Result<Box<dyn Stream>, String>; fn meeting_target(&self) -> Option<u32>; fn list_devices(&self) -> (Vec<DeviceInfo>, Vec<DeviceInfo>); }` (the tuple is system render devices, then microphones)
  - `#[cfg(test)] pub(crate) struct FakeBackend`: lets a test push `Msg` values per `Source`, set the meeting target, make a given `SourceKind` variant fail to open, and read which `SourceKind`s were opened.
- Produces in `worker.rs`:
  - `pub struct WorkerConfig { pub retry_interval: Duration }` with `Default` of 2 s (tests use 50 ms)
  - `pub struct AudioHandle`; `AudioHandle::spawn(backend: Arc<dyn Backend>, config: WorkerConfig) -> AudioHandle`; `send(&self, c: Command)`; `try_events(&self) -> Vec<Event>`; dropping the handle stops the worker and finalizes an active session.
- Worker rules implemented here: `Start` refused with `Error` when both sources are off; a source that fails to open emits `SourceStatus { Failed(msg) }` and the other keeps going; both fail gives `Error` and removes the empty session folder; `Start` creates the folder with `naming::session_dir` and `naming::local_stamp`, one `WavWriter` + `TrackClock` per source; every chunk goes through `TrackClock::place`; header flush every 5 s of recorded time; `Pause` pads both tracks to the larger written length then discards chunks; the first chunk after `Resume` calls `TrackClock::resume`; `Stop` pads both to the larger length, finishes writers, emits `Stopping`, `Finalizing`, `Saved { dir, duration }`; `Levels` emitted at most every 50 ms of recorded time from `dbfs` of the latest chunk (Disabled sources report -90); `Elapsed` derived from frames written, not wall-clock; a writer error or a full writer emits `Error`, still finalizes the other track, and never panics; `ListDevices` emits `Devices`.

- [ ] **Step 1: Write the failing tests (in `worker.rs`, using `FakeBackend` and `audio::temp_dir`)**

  - `start_pause_resume_stop_writes_two_equal_files`: push 1 s of samples to both sources, Pause, push 1 s more (must be discarded), Resume, push 1 s, Stop; `Saved` arrives; both files are 2 s (96,000 frames), equal length, valid headers; the discarded second never appears.
  - `silent_system_source_still_yields_equal_length_files`: only the mic receives 3 s of chunks, the system source receives nothing; after Stop both files are 3 s long and the system file is all zeros; no `Failed` status was emitted for System.
  - `late_first_chunk_is_front_filled`: system's first chunk starts at frame 24,000 (0.5 s); the file begins with 24,000 zeros.
  - `both_sources_disabled_is_refused`: `Start` with `system: false, mic: false` yields `Error`, state not Recording, no folder.
  - `one_source_failing_keeps_the_other`: `FakeBackend` fails `Mic` open; `SourceStatus { Mic, Failed(_) }`, recording continues with System, `Saved` after Stop, only `system.wav` has data.
  - `neither_source_starting_is_error_and_leaves_no_folder`.
  - `unwritable_dir_goes_to_error`: `base_dir` is an existing regular file; `Error` event, no panic, state returns to a restartable one.
  - `levels_follow_chunk_loudness`: loud chunk gives a `Levels` value above -20; silence gives -90.
  - `list_devices_emits_devices_event`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::worker`
Expected: FAIL, types not defined.

- [ ] **Step 3: Implement `backend.rs` and `worker.rs`**

One worker thread; `recv_timeout` on a combined loop over the command receiver and the chunk receiver (tests need determinism, so process everything already queued before idling). Write silence in 1-second pieces so a long gap does not allocate hours of zeros at once.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::worker` then fmt, clippy, full test.
Expected: PASS (9 tests).

- [ ] **Step 5: Commit**

```bash
git add src/audio/backend.rs src/audio/worker.rs src/audio/mod.rs
git commit -m "feat(audio): add backend trait and session worker core"
```

---

### Task 6: Worker resilience (fallback, lost sources, odd stops)

**Files:**
- Modify: `src/audio/worker.rs`

**Interfaces:**
- Consumes: Task 5 types.
- Behavior added:
  - On `Start` the System source asks `backend.meeting_target()`; `Some(pid)` opens `SourceKind::Process { pid }`; `None` opens `SourceKind::WholeSystem` and emits `SourceStatus { System, AllSystemFallback }`. A fallback session never switches to a process mid-recording.
  - `Msg::Lost(System)` on a process source: status `WaitingForApp`; every `retry_interval` the worker calls `meeting_target()` and, when `Some`, reopens `Process` and emits `Active`. The gap is silence (the clock fills it).
  - `Msg::Lost(Mic)`: status `DeviceLost`; reopen every `retry_interval`; `Active` when it returns.
  - A lost `WholeSystem` source is retried the same way as the mic.

- [ ] **Step 1: Write the failing tests (worker.rs)**

  - `no_meeting_app_falls_back_to_whole_system`: target `None`; opened kinds contain `WholeSystem`, status `AllSystemFallback` was emitted.
  - `meeting_app_found_opens_process_source`: target `Some(42)`; opened kinds contain `Process { pid: 42 }`, no fallback status.
  - `fallback_does_not_switch_mid_recording`: start in fallback, set a target afterwards, wait several retry intervals, no `Process` open happens.
  - `app_exit_waits_then_reattaches`: `Lost(System)` gives `WaitingForApp`; setting the target to `Some(7)` gives a `Process { pid: 7 }` open and `Active` within a few retry intervals; the system file length equals the mic file length at Stop.
  - `mic_lost_continues_system_and_reattaches`: `Lost(Mic)` gives `DeviceLost`, system keeps writing, mic reopens, final lengths equal.
  - `stop_while_paused_finalizes_equal_length`: Pause, then Stop; `Saved` arrives; equal lengths.
  - `stop_while_source_lost_finalizes_equal_length`: `Lost(Mic)` then Stop without reattach; `Saved` arrives, equal lengths, mic padded with silence.
  - `drop_handle_mid_recording_finalizes_files`: dropping `AudioHandle` leaves playable files (both headers correct).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::worker`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement the fallback and retry logic in `worker.rs`**

Track per-source state (`Active`, `Waiting`, `Lost`) and a next-retry `Instant` for each; no thread per retry.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::worker` then fmt, clippy, full test.
Expected: PASS (17 tests).

- [ ] **Step 5: Commit**

```bash
git add src/audio/worker.rs
git commit -m "feat(audio): add fallback, source retry and safe stop handling"
```

---

### Task 7: WASAPI backend (Windows)

**Files:**
- Create: `src/audio/wasapi_backend.rs`
- Modify: `src/audio/mod.rs` (`#[cfg(windows)] pub mod wasapi_backend;`), `Cargo.toml` (`wasapi` under `[target.'cfg(windows)'.dependencies]`, latest published version, record it in `DECISIONS.md` later)

**Interfaces:**
- Consumes: `Backend`, `SourceKind`, `Chunk`, `Msg`, `Stream`, `DeviceInfo` (Tasks 4 and 5), `procfind::{list_processes, pick_target}`.
- Produces: `pub struct WasapiBackend; impl WasapiBackend { pub fn new() -> Self }`, `impl Backend for WasapiBackend`.
  - `open(Mic)`: shared-mode capture on the chosen or default capture device; `open(WholeSystem)`: loopback on the chosen or default render device; `open(Process{pid})`: application loopback with the process tree. Each runs on its own thread, converts to mono i16 at the requested rate, converts device timestamps to frames since `origin`, sends `Msg::Chunk`, and sends `Msg::Lost` once when the stream or device dies; `Stream::stop` ends the thread and waits for it.
  - `meeting_target()`: `procfind::pick_target(&list_processes(), &active_audio_pids())`, where `active_audio_pids()` returns pids with an active audio session (empty set if the API is not available in the pinned crate).
  - `list_devices()` returns render devices (for loopback) and capture devices, each with `is_default` set.
  - Pure helper `pub(crate) fn to_mono_i16(data: &[u8], channels: u16, format: SampleFormat) -> Vec<i16>` for `F32` and `I16`/`I32` interleaved input (average the channels, clamp), used when the device cannot convert for us.
  - Pure helper `pub(crate) fn frames_since(origin: Instant, now: Instant, sample_rate: u32) -> u64`.

- [ ] **Step 1: Write the failing tests for the pure helpers**

`to_mono_i16_f32_stereo_averages` (`[0.5, -0.5]` frame gives 0; `[1.0, 1.0]` gives `i16::MAX`), `to_mono_i16_clamps_overrange` (`[2.0, 2.0]` gives `i16::MAX`), `to_mono_i16_i16_mono_passthrough`, `frames_since_matches_rate` (500 ms at 48,000 Hz gives 24,000).

Hardware tests, marked `#[ignore]` (run by hand with `cargo test -- --ignored wasapi_`): `wasapi_lists_devices` (both lists non-empty, exactly one default each), `wasapi_mic_captures_two_seconds` (open default mic at 48,000 Hz, collect for 2 s, total frames between 80% and 120% of 96,000), `wasapi_whole_system_loopback_opens`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test audio::wasapi_backend`
Expected: FAIL, helpers not defined.

- [ ] **Step 3: Implement the helpers, then the backend**

First confirm against the pinned crate version: the application-loopback constructor name (`new_application_loopback_client(process_id, include_tree)` versus `ProcessAudioClient`), whether shared-mode auto-conversion to the requested format works for device capture, loopback and process loopback, and whether per-process audio sessions can be enumerated. If any cannot be done with the crate, use the `windows` COM bindings for that one call and note it in `DECISIONS.md`. If process loopback needs a fixed format, capture in that format and resample to the session rate with linear interpolation (an `fn resample_linear(input: &[i16], from: u32, to: u32) -> Vec<i16>` helper with its own test).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test audio::wasapi_backend` (pure tests), then manually `cargo test -- --ignored wasapi_` on the target machine with a microphone present; then fmt, clippy, full test.
Expected: pure tests PASS; ignored tests PASS on this machine. Record in the commit body whether process loopback was exercised (it needs Zoom or Teams playing; do it in Task 10).

- [ ] **Step 5: Commit**

```bash
git add src/audio/wasapi_backend.rs src/audio/mod.rs Cargo.toml Cargo.lock
git commit -m "feat(audio): add WASAPI backend for mic, loopback and process capture"
```

---

### Task 8: Device lists and saved device IDs

**Files:**
- Modify: `src/settings_store.rs`, `src/app.rs`, `src/screens/settings/audio.rs`, `src/screens/recorder.rs` (device dropdowns only), `src/i18n.rs`

**Interfaces:**
- Consumes: `AudioHandle::send(Command::ListDevices)` and `Event::Devices`, `DeviceInfo`.
- Produces:
  - `Persisted` gains `system_device: String` and `mic_device: String` (empty string means the Windows default), stored as `audio.system_device` and `audio.mic_device`.
  - `pub fn selected_index(devices: &[DeviceInfo], id: &str) -> usize` in `settings/audio.rs`: 0 is "Windows default"; entries 1..=n are the devices; an unknown id returns 0.
  - `AppState` holds one shared `devices: DeviceLists` (system and mic `Vec<DeviceInfo>`) and the two selected ids; the Settings > Audio dropdowns and the Recorder screen's dropdowns read and write the same ids (the separate mock lists and `*_device_idx` fields are removed).
  - `RecorderScreen` gains `pub audio: Option<AudioHandle>` (the single owner of the handle) and `pub fn take_events(&self) -> Vec<Event>`.
  - `AppState::enable_audio(&mut self, handle: AudioHandle)` stores the handle in `self.recorder.audio` and sends `ListDevices` once; `AppState::apply_audio_event(&mut self, e: Event)` routes `Event::Devices` to `self.devices` and every other event to `recorder.apply_event` (added in Task 9; until then other events are ignored). Dev-flag runs and tests have no handle and show a single "Windows default" entry.

- [ ] **Step 1: Write the failing tests**

`settings_store`: `device_ids_round_trip` (ids such as `{0.0.1.00000000}.{abc-123}` survive save and load), `missing_device_keys_default_to_empty`, existing tests still pass. `audio.rs`: `selected_index_default_for_empty_and_unknown`, `selected_index_finds_device_after_default_entry`. `app.rs`: `devices_event_fills_lists` (feed `Event::Devices` through the method that applies events; lists update and a saved id absent from the list keeps its stored value but selects index 0). `i18n`: `new_device_labels_have_thai` ("Windows default" and any other new label).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test settings_store audio app i18n`
Expected: FAIL.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test` then fmt and clippy. Screenshot Settings > Audio and Recorder in dark and light, English and Thai, with `--screen=settings-audio` and `--screen=recorder`; compare with the previous look (dropdown width must not break layout with long device names).
Expected: PASS and no layout regressions.

- [ ] **Step 5: Commit**

```bash
git add src/settings_store.rs src/app.rs src/screens/settings/audio.rs src/screens/recorder.rs src/i18n.rs
git commit -m "feat(audio): list real devices and persist selected device IDs"
```

---

### Task 9: Recorder UI wired to the worker

**Files:**
- Modify: `src/screens/recorder.rs`, `src/app.rs`, `src/shell.rs`, `src/main.rs`, `src/i18n.rs`, `src/screens/settings/audio.rs` (sample-rate mapping helper if needed)

**Interfaces:**
- Consumes: `AudioHandle`, `Command`, `Event`, `StartRequest`, `SessionState`, `SourceStatus`, selected device ids (Task 8).
- Produces:
  - `RecorderState` gains `Starting` and `Saved`; `Stopping` and `Finalizing` both show as the existing `Processing`. All matches (`label`, `state_badge_style`, `shell.rs` state list, `--state=` flag) are updated.
  - `RecorderScreen.audio: Option<AudioHandle>`; `None` is preview mode, where `toggle_recording` and `toggle_pause` behave exactly as today (existing tests unchanged).
  - `RecorderScreen::apply_event(&mut self, e: Event)` updates `state`, `sys_audio_level`/`mic_level` (dBFS), elapsed text, per-source status text, `last_error: Option<String>`, and saved folder.
  - `RecorderScreen::start_request(&self, sample_rate: u32, system_device: Option<String>, mic_device: Option<String>) -> StartRequest` from `sys_audio_enabled`, `mic_enabled`, `destination`, `file_name`.
  - `timer_text(&self) -> String` and `global_label(&self) -> String` replace the static mock strings when a session exists; preview mode keeps `00:14:32` and `00:00:00`.
  - Transport buttons call `toggle_recording`/`toggle_pause`/a stop method instead of assigning `screen.state` directly; the shortcuts in `app.rs` keep working.
  - `AppState::update` drains `recorder.take_events()` each frame into `AppState::apply_audio_event` and calls `request_repaint_after(50 ms)` while the state is Starting, Recording, Paused, Stopping or Finalizing.
  - `main.rs` creates `WasapiBackend` and `AudioHandle` only for normal runs (not dev-flag runs), mirroring how persistence is enabled.
  - Fallback badge "Capturing all system audio" is shown while the System status is `AllSystemFallback`; `WaitingForApp` and `DeviceLost` show their own notices; the error banner shows `last_error` instead of the hard-coded text.

- [ ] **Step 1: Write the failing tests (recorder.rs, app.rs)**

  - `apply_state_events_map_to_recorder_state` (Starting, Recording, Paused, Stopping and Finalizing map to `Processing`, Saved, Error).
  - `apply_levels_updates_meters`.
  - `apply_error_sets_banner_message_and_state`.
  - `apply_saved_records_folder_and_toast_text`.
  - `start_request_uses_screen_fields` (toggles, destination, name; empty name stays empty for `naming` to fix).
  - `preview_mode_toggles_unchanged` (the existing `toggle_recording_starts_and_stops` and pause tests pass without a handle).
  - `timer_text_formats_elapsed` (3,725 s gives `01:02:05`).
  - `new_status_labels_have_thai`.
  - `app.rs`: `stop_shortcut_in_session_sends_stop_command` using a handle built on `FakeBackend`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test`
Expected: FAIL on the new tests.

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test` then fmt and clippy.
Then run the real app (`cargo run`), press Record, speak for 5 seconds with a microphone present, play any sound, Pause for 3 seconds, Resume, Stop: confirm the status, timer and meters move, the folder appears, and both WAV files open in a player. Screenshot the Recorder in Idle, Recording, Paused, fallback badge and error states.
Expected: automated PASS; the manual run behaves as described.

- [ ] **Step 5: Commit**

```bash
git add src/screens/recorder.rs src/app.rs src/shell.rs src/main.rs src/i18n.rs src/screens/settings/audio.rs
git commit -m "feat(audio): wire Recorder UI to the capture worker"
```

---

### Task 10: Acceptance on a real meeting and docs

**Files:**
- Modify: `TASKS.md`, `ROADMAP.md`, `DECISIONS.md`, `design.md` (section 15), `UI_PROGRESS.md` if it lists the mock recorder

**Interfaces:**
- Consumes: the finished feature. No new code.

- [ ] **Step 1: Run the acceptance checklist from the spec on this machine**

Record a 2-minute Zoom or Teams call and tick each result: both WAVs play; other participants only in `system.wav` and the user only in `mic.wav`; lengths differ by at most 100 ms; a 30-second Pause is absent from both files; killing the app mid-recording leaves playable files (run `recover_header` on them if a player refuses); starting without Zoom or Teams shows "Capturing all system audio"; quitting and reopening Zoom resumes the system track; unplugging the microphone keeps recording and the UI says so; the UI never stalls. Record every result and every failure honestly in `TASKS.md` evidence.

- [ ] **Step 2: Update the documents**

`TASKS.md`: move the audio capture work to Completed only for acceptance items that passed, with the evidence; list failed or untested items as open tasks. `ROADMAP.md`: tick "Audio capture" only if every acceptance item passed. `DECISIONS.md`: entries for the `wasapi` crate (version, any COM fallback), per-process loopback with whole-system fallback, tolerance and header-flush values, worker-computed levels, and the "idle app may win" risk. `design.md` section 15: move audio capture out of "Not done", and add the new states and the fallback badge as implemented behavior.

- [ ] **Step 3: Verify and commit**

Run: `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test`
Expected: all pass.

```bash
git add TASKS.md ROADMAP.md DECISIONS.md design.md UI_PROGRESS.md
git commit -m "docs: record audio capture acceptance results and decisions"
```
