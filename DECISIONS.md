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
