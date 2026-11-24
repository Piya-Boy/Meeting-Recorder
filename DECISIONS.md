# Technical Decision Log

Record decisions affecting architecture, dependencies, data formats, security, and maintenance.

## Decision Template

### YYYY-MM-DD - Decision Title

- Status: Proposed / Accepted / Superseded
- Context:
- Decision:
- Alternatives:
- Consequences:
- References:

---

## Decisions

_Add decisions below._

### 2026-11-24 - Native egui UI, no WebView

- Status: Accepted
- Context: Stitch designs are HTML; the app must be a native Rust desktop app.
- Decision: Re-implement every screen in egui/eframe 0.31; use Stitch HTML renders only as visual references.
- Alternatives: WebView wrapper (rejected: user constraint).
- Consequences: Visual parity needs manual tuning (fonts, weights, spacing). No CSS features such as variable font weights.
- References: UI_PROGRESS.md

### 2026-11-24 - Bundle fonts (Inter, JetBrains Mono, Noto Sans Thai) with PUA stripped

- Status: Accepted
- Context: System fonts did not match the design; Inter / JetBrains contain private-use glyphs that shadow Phosphor icons.
- Decision: Embed OFL fonts via `include_bytes!`, one egui family per weight, strip PUA from the Latin fonts.
- Alternatives: System fonts (inconsistent look).
- Consequences: Larger binary; licences kept in `assets/fonts`.
- References: `src/theme.rs`

### 2026-11-24 - Frameless window with custom title bar

- Status: Accepted
- Context: OS title bar had a different colour and a title string the design does not show.
- Decision: `with_decorations(false)`; own drag region, window controls, edge resize (`BeginResize`), DWM rounded corners.
- Alternatives: Native decorations (rejected by user).
- Consequences: No Windows 11 snap-layout flyout on the maximise button; Windows-specific code behind `cfg(windows)`.
- References: `src/shell.rs`, `src/main.rs`

### 2026-11-24 - UI language via English-keyed dictionary

- Status: Accepted
- Context: Settings has an "Application language" dropdown (English / Thai).
- Decision: `i18n::tr(&str)` looks up the English text in a Thai table and falls back to the input; language is a process-global set each frame. Translation sits in shared components.
- Alternatives: Key-based i18n crate (heavier; every call site changes).
- Consequences: Missing entries show English; the choice is not persisted yet.
- References: `src/i18n.rs`

### 2026-11-24 - Text-specific semantic colours in light theme

- Status: Accepted
- Context: Stitch's bright green / red / blue is poor contrast as text on white.
- Decision: Add `success_text`, `error_text`, `link_text` (darker in light, equal to base in dark); use bright colours only for fills, dots and meters.
- Alternatives: Darken the base colours (would dull meters and fills).
- Consequences: Slight deviation from Stitch in light theme, intentional.
- References: `src/theme.rs`

### 2026-11-24 - Preferences in a plain key=value file

- Status: Accepted
- Context: Theme / language / accent must survive restarts (design 5.9).
- Decision: `%APPDATA%\MeetingRecorder\settings.ini`, atomic write, written only when a preference changes; dev-flag runs never touch it.
- Alternatives: eframe persistence feature (pulls serde, stores the whole app state); registry.
- Consequences: No new dependency; unknown keys tolerated; failures are silent so a read-only profile cannot break the app.
- References: `src/settings_store.rs`

### 2026-11-24 - Executable icon through build.rs

- Status: Accepted
- Context: The window icon is set at runtime, but Explorer and taskbar pins read the exe resource.
- Decision: `winresource` build dependency embeds `assets/icon/app.ico`; build warns instead of failing when no resource compiler exists.
- Alternatives: Skip the exe icon.
- Consequences: Build needs the Windows SDK `rc.exe` to get the icon (present here).
- References: `build.rs`

### 2026-11-24 - WASAPI through the `wasapi` crate

- Status: Accepted
- Context: Capture needs microphone, whole-system loopback and per-process loopback (Windows 10 2004+ / Windows 11).
- Decision: `wasapi` 0.27.0 (Windows only, `cfg(windows)`), event-driven shared mode with `autoconvert` and a 20 ms buffer; a render device opened as a capture client is the whole-system loopback. `windows-sys` stays for Toolhelp (process list), local time and thread calls. No raw COM fallback was needed.
- Alternatives: `windows` crate with hand-written COM; `cpal` (no process loopback).
- Consequences: One extra Windows-only dependency; per-process loopback needs Windows 10 2004 or later (older builds fall back to whole-system). Process loopback opened on this machine (Windows 11 build 26300); capturing a real call is not yet verified.
- References: `src/audio/wasapi_backend.rs`, `Cargo.toml`

### 2026-11-24 - Per-process loopback with whole-system fallback

- Status: Accepted
- Context: The system track should hold the meeting app, not notification sounds or music.
- Decision: The meeting target is the Zoom.exe / ms-teams.exe / Teams.exe process that has an active audio session, else the lowest PID of those; no meeting process (or a failed open) means whole-system capture with the status "All system audio" on the Recorder. A process source that exits becomes "waiting for the meeting app" and is retried every 2 s.
- Alternatives: Whole-system only (simple, but records unrelated sounds); ask the user to pick a window.
- Consequences: Known risk: with Zoom and Teams both open and neither playing audio, the lowest PID wins, which may be the wrong app. Not verified with a real call yet.
- References: `src/audio/procfind.rs`, `src/audio/worker.rs`

### 2026-11-24 - Track timing and file safety values

- Status: Accepted
- Context: The two tracks must stay aligned, survive Pause and device gaps, and stay readable after a crash.
- Decision: Each track places chunks by device timestamp and pads gaps with silence when they exceed 10 ms (rate / 100); Pause pads both tracks to the same length then discards audio; Resume rebases both clocks on the first chunk; the WAV header is rewritten every 5 s of recorded audio; Stop pads to the longest track. Files are mono 16-bit PCM at the Settings rate, one per source, in `<destination>\<YYYY-MM-DD_HHMM>_<name>\`; a source that fails or is off leaves no file.
- Alternatives: One stereo file (the tracks could not be separated or transcribed apart); rewriting the header only on Stop (a crash would leave unplayable files).
- Consequences: After a crash a header lists audio only up to the last flush (measured: 5.01 s of 6.99 s); `recover_header` can repair this but is not wired in yet.
- References: `src/audio/clock.rs`, `src/audio/wav.rs`, `src/audio/worker.rs`

### 2026-11-24 - Levels computed in the worker

- Status: Accepted
- Context: The UI must not touch audio samples or wait on capture threads.
- Decision: The worker computes dBFS (RMS, floor -90) and sends `Levels` at least every rate / 20 frames (50 ms); the UI only receives events through a channel and polls them each frame, repainting every 50 ms while a session is active.
- Alternatives: Share a sample buffer with the UI (locking, copies).
- Consequences: Meters cost one small event per 50 ms; the UI has no audio dependency, so it can run with a fake backend in tests.
- References: `src/audio/session.rs`, `src/app.rs`

### 2026-11-24 - ONNX Runtime through the `ort` crate

- Status: Accepted
- Context: OpenThai 2.0 ASR Streaming ships as ONNX files; the app is native Rust and has no Python.
- Decision: `ort` 2.0.0-rc.15 with `default-features = false` and features `std`, `tls-native`, `api-31`, `download-binaries`. The build script downloads the prebuilt ONNX Runtime and links it into the exe (no separate DLL sits next to the debug exe). One shared `Environment` per process; one session for the encoder, one for the decoder/joiner. Session builder steps are mapped one by one so clippy's `result_large_err` stays quiet.
- Alternatives: `onnxruntime-sys` directly (more code, no safe wrapper); calling a Python sidecar (rejected: extra runtime to install); `tract` (no support for the model's operators was checked).
- Consequences: A release build needs network access once, to fetch the runtime. `ort` is a release candidate, so a version bump may change the API. The encoder takes 1.1 GB of RAM (int8, measured), so the worker frees it after 5 minutes without a job.
- References: `Cargo.toml`, `src/asr/engine.rs`

### 2026-11-24 - Model delivery: pinned revision, hashes, download on first use

- Status: Accepted
- Context: The model is about 1 GB (int8) or 2.6 GB (fp32); it cannot sit in the repository or the installer, and a download must not be able to put a wrong or half-written file in use.
- Decision: The app downloads from `https://huggingface.co/iapp/OpenThai2.0-ASR-Streaming-ONNX/resolve/<revision>/` with the revision pinned to `9f142a43e4d35a422c1c7dac40772ef707387ebc`. `asr::model::manifest` lists every file with its size and SHA-256; a file is used only after its hash matches. Download goes to `<name>.part`, resumes with a Range request, retries up to 6 times, treats 45 s without data as a stall, and can be cancelled. The zip (encoder graph, `decoder_joint.onnx`, `tokens.txt`) is extracted through a four-name whitelist. `ready.ok` holds `size=<bytes of the weights file>`; `status` trusts it instead of hashing 1 GB at every start. Folder: `%LOCALAPPDATA%\MeetingRecorder\models\9f142a4\<int8|fp32>`. HTTP: `ureq` 3 with `native-tls` (Windows' own TLS stack, no extra certificate bundle); `zip` 9 and `sha2` 0.11.
- Alternatives: Bundle the model (too big, and the licence question below); fetch the latest revision (a silent model change would change results); hash on every start (seconds of disk I/O).
- Consequences: First use needs about 1 GB of download. A model folder edited by hand (same size file) is trusted. A new model revision means a new manifest and a new folder. Measured here: 946 MiB downloaded and verified in about 40 s (debug build, where the hash check is slow).
- References: `src/asr/model.rs`, `src/asr/download.rs`

### 2026-11-24 - Transcription code ported from PoodPim (MIT)

- Status: Accepted
- Context: Log-mel features, the silence segmenter, greedy RNN-T decoding, resumable download and the resampling kernel already exist in PoodPim and were checked against the same model.
- Decision: Port them into `src/asr/` with a header in each ported file naming the source and commit (`15e07cc835a6b16b95bea65ae31c3a2eb4c6e856`), and keep PoodPim's MIT text in `THIRD_PARTY_NOTICES.md`.
- Alternatives: Write them from the model card (slower, no reference to compare with); depend on PoodPim as a crate (it is an application).
- Consequences: The ported code keeps PoodPim's structure. MIT is compatible with the project's licence; the notice must stay with any distribution.
- References: `THIRD_PARTY_NOTICES.md`, `src/asr/features.rs`, `src/asr/segment.rs`, `src/asr/engine.rs`

### 2026-11-24 - Segmentation and decoding values

- Status: Accepted
- Context: The model reads at most 30 s per call and is trained on speech, not on silence; meetings have long quiet parts.
- Decision: Audio is converted to 16 kHz mono first. The segmenter uses PoodPim's cut rules on a fixed 30 s window of pending audio (so cuts do not depend on how the audio arrives): a pause of at least 0.5 s cuts once the segment is 8 s long; a pause of at least 1.2 s cuts once it is 2 s long; with no pause by 25 s it cuts at the quietest 0.2 s of the last 6 s. A frame is quiet below -50 dBFS (20 ms frames); each segment is trimmed to its sound with 0.25 s padding; segments shorter than 0.3 s are dropped. Decoding is greedy with at most 10 symbols per frame; the blank is biased by -1.0 (-2.5 under 2 s of audio); a pause between tokens becomes a space; the sara am is normalised (U+0E4D U+0E32 becomes U+0E33). A track at or below the quiet threshold the whole time gives no lines and never reaches the model.
- Alternatives: Fixed 30 s chunks (cuts words); a neural voice detector (another model to ship).
- Consequences: A very quiet microphone (below -50 dBFS) is treated as silence. Measured on 10 FLEURS Thai clips with int8, 4 threads: CER 5.27% (bar 6%); the same clips converted to 48 kHz and resampled by the app: 4.84%.
- References: `src/asr/segment.rs`, `src/asr/engine.rs`, `src/asr/thai.rs`, `src/asr/accuracy.rs`

### 2026-11-24 - Track labels: microphone is "You", system audio is "Others"

- Status: Accepted
- Context: There is no speaker separation model; the two tracks are the only speaker information.
- Decision: Lines from `mic.wav` are "You", lines from `system.wav` are "Others"; both tracks are transcribed alone and merged by start time (microphone first on ties). `transcript.txt` uses the names of the UI language at start-up; `transcript.json` stores `"track": "mic" | "system"`.
- Alternatives: Speaker diarization (not available in this release); a mixed mono track (loses who spoke).
- Consequences: Several people in the room share "You"; several remote people share "Others". A person whose voice comes out of the speakers and is picked up by the microphone appears on both tracks (no echo handling).
- References: `src/asr/transcript.rs`, `src/asr/worker.rs`

### 2026-11-24 - Recorder footer shows a line count, not a word count

- Status: Accepted
- Context: The mock footer said "142 Thai words". Thai has no spaces between words; the model's spaces mark pauses, so counting them says nothing about words.
- Decision: The footer shows the number of lines ("N lines"); no word count.
- Alternatives: Word segmentation with a dictionary (another dependency and model).
- Consequences: Less information in the footer; nothing misleading.
- References: `src/screens/recorder.rs`

### 2026-11-24 - Transcription worker: one job at a time, nothing half-written

- Status: Accepted
- Context: Transcription takes seconds to minutes and must neither freeze the window nor disturb a recording.
- Decision: One worker thread at below-normal priority, commands and events over channels (as for audio). Jobs run in order, one per folder; Stop on a recording sends a job at once (or notes that the model is missing). Cancel and close stop between two segments. `transcript.json` and `transcript.txt` are written once at the end, each through a temporary file and a rename, so a cancelled, failed or killed job leaves neither; the WAV files are never written. The loaded model is dropped after 5 minutes without a job and before the model files are deleted. Threads: 4, 6 or 8 (default 4).
- Alternatives: Writing the transcript while it grows (a crash leaves a partial file); one thread per job (several 1 GB models in memory).
- Consequences: A cancel sent before its job starts is ignored; only the running job can be cancelled. ONNX Runtime's own threads keep normal priority (see `TASKS.md`).
- References: `src/asr/worker.rs`

### 2026-11-24 - Open licence gate: model licence and the "Powered by OpenThai ASR" badge

- Status: Proposed (unresolved, blocks any release that bundles or ships the model)
- Context: The model is under OpenMDW-1.1 (text in `THIRD_PARTY_NOTICES.md`); the project has its own licence (`LICENSE`). The UI shows a "Powered by OpenThai ASR"-style badge.
- Decision: None yet. The app only downloads the model onto the user's own computer from the publisher's repository and does not redistribute it. Nobody has checked (1) that OpenMDW-1.1 and the project's licence work together for a release, or (2) whether the model's publisher sets rules for the badge.
- Alternatives: Ask the publisher; take legal advice; drop the badge.
- Consequences: Do not publish an installer that includes the model, or ship the badge as final, before this is settled.
- References: `THIRD_PARTY_NOTICES.md`, `LICENSE`

### 2026-11-24 - Recordings and Transcripts scan the folders; no SQLite index yet

- Status: Accepted
- Context: The screens must show what is on disk. The session folders already hold everything (WAV headers, `transcript.json`).
- Decision: `library::scan` reads every direct subfolder of the Recorder's destination that holds `mic.wav` or `system.wav`, on a short background thread; the newest request wins. It runs when Recordings or Transcripts is opened, after a save, after a transcription ends and after a delete. A transcript counts only if `transcript.json` is version 1.
- Alternatives: A SQLite index (fast search over everything, but it must stay in sync with folders the user changes in Explorer and needs a repair path); scanning on the UI thread (stalls on big folders).
- Consequences: The list never disagrees with the disk after a refresh; a folder changed in Explorer shows only when the screen is opened again. 200 sessions scan in 37 ms (debug build), so an index is not needed yet; revisit when search over thousands of transcripts is slow.
- References: `src/library.rs`, `docs/superpowers/specs/2026-11-24-library-real-data-design.md`

### 2026-11-24 - Delete removes only what the app wrote

- Status: Accepted
- Context: The destination folder belongs to the user; a delete button must not remove their other files.
- Decision: `delete_session` removes `mic.wav`, `system.wav`, `transcript.json`, `transcript.txt` and their `.tmp` files, then removes the folder only if it is empty; it refuses any path that is not a direct child of the destination. The app refuses a delete while a recording is in progress or while that folder is being transcribed.
- Alternatives: `remove_dir_all` (simple, would delete a user's notes in the same folder); the recycle bin (needs another Windows API; the modal already says it cannot be undone).
- Consequences: A folder with the user's own files stays on disk, without its audio; it no longer lists because no WAV is left. Deletion is permanent.
- References: `src/library.rs`, `src/app.rs`

### 2026-11-24 - `serde_json` for reading `transcript.json`

- Status: Accepted
- Context: The library must read the transcript files the app writes; a hand-written parser for escapes and nesting is a risk for little gain.
- Decision: Add `serde_json` 1 and read `transcript.json` through `serde_json::Value`. The writer stays hand-written.
- Alternatives: Parse `transcript.txt` (labels depend on the UI language at the time); a hand-written JSON reader.
- Consequences: One more dependency (`serde` and `serde_json` were already in the lock file through other crates).
- References: `Cargo.toml`, `src/library.rs`

### 2026-11-25 - Exports go next to the recording, no Save dialog

- Status: Accepted
- Context: The Export JSON / SRT / TXT buttons had no behaviour. The library already keeps everything about a recording in its own folder.
- Decision: An export writes (or returns) the file in the recording's folder and shows it selected in Explorer (`explorer /select,"<path>"`). `transcript.json` and `transcript.txt` are the files the transcription wrote and are never overwritten; `transcript.srt` is derived and rewritten each time; the app's delete removes it.
- Alternatives: A native Save dialog through the `rfd` crate (new dependency, and a blocking call that needs its own thread and a channel); clipboard only (already exists as Copy).
- Consequences: No choice of destination: the user copies the file from Explorer. An existing `transcript.txt` keeps the labels of the language it was written in. Revisit with a Save dialog if users ask.
- References: `src/export.rs`, `docs/superpowers/specs/2026-11-25-transcript-export-design.md`
