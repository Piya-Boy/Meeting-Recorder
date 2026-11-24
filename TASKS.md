# Task Tracker

## In Progress

_None._

## Next

- [ ] Task: Audio capture acceptance on a real Zoom or Teams call (open items of the audio capture work)
  - Acceptance Criteria (not yet run, all need a real call or hardware change): in a 2-minute call the other participants are only in `system.wav` and the user only in `mic.wav`; quitting and reopening Zoom/Teams resumes the system track; unplugging the microphone keeps recording and the UI says so; with Zoom and Teams both open and idle the right one is picked (known risk: lowest PID wins).
  - Open from the checks that did run: after a crash the WAV headers list only the audio up to the last 5 s flush; `recover_header` exists and is tested but nothing calls it (no start-up repair, no UI).
  - Verification: record the call, then inspect both WAVs; record the result per item here.
- [ ] Task: Wire the UI to the remaining real backends (OpenThai ASR / ONNX, SQLite)
  - Acceptance Criteria: live transcript from ASR; recordings and transcripts read from SQLite instead of mock data.
  - Verification: Record a short session, reopen the app, find it in Recordings and Transcripts; `cargo test` covers the storage layer.
- [ ] Task: Keyboard navigation inside tables and dropdown popups
  - Acceptance Criteria: Arrow keys / Enter work in rows and popups; focus ring visible.
  - Verification: Real key presses with screenshots.

## Blocked

_None._

## Completed

- [x] Task: Implement all 10 Stitch screens and 3 modals as native egui UI
  - Verification Evidence: every screen screenshotted from the running exe and compared with the Stitch render; real-click checks for navigation, delete modal, sidebar collapse, Appearance. See `UI_PROGRESS.md`.
  - Commit: not committed (repository has no commits; user has not asked for one).
- [x] Task: Restyle buttons, dropdowns, playback bar to the Stitch spec; bundle Inter / JetBrains Mono / Noto Sans Thai
  - Verification Evidence: screenshots vs Stitch; fonts and OFL licences in `assets/fonts`.
  - Commit: not committed.
- [x] Task: Frameless window with custom title bar, drag, resize, rounded corners, inset window controls
  - Verification Evidence: scripted real mouse tests (drag, maximise, restore, close, edge/corner resize, 1200x760 minimum).
  - Commit: not committed.
- [x] Task: Light-theme legibility (button text bug, darker semantic text colours, lighter font weights)
  - Verification Evidence: light-theme screenshots of all screens re-reviewed.
  - Commit: not committed.
- [x] Task: English / Thai UI language switching (`src/i18n.rs`)
  - Verification Evidence: all 10 screens and Delete/Help modals shot in Thai; live switch via the dropdown verified by clicking; unit test for lookup.
  - Commit: not committed.
- [x] Task: App logo as window icon, sidebar logo and About logo (`src/branding.rs`)
  - Verification Evidence: zoomed screenshots (dark and light); unit tests for transparent backdrop and icon size. The exe icon was verified separately (see the exe icon task).
  - Commit: not committed.
- [x] Task: Persist user preferences (theme, accent, font scale, language, general toggles)
  - Verification Evidence: unit tests (round trip, bad input); two real launches: language chosen in the dropdown was restored on relaunch; dev-flag runs neither read nor write the file.
  - Commit: not committed.
- [x] Task: App-wide shortcuts (Ctrl+Shift+R / P / A / C)
  - Verification Evidence: real key presses: Ctrl+Shift+R then P produced the Paused state; unit tests for the state transitions.
  - Commit: not committed.
- [x] Task: Transcription engine loading / ready / unavailable states
  - Verification Evidence: screenshots with `--engine=loading|error` on the Recorder and Settings > Transcription; unit tests for the mock load.
  - Commit: not committed.
- [x] Task: Keyboard focus ring
  - Verification Evidence: Tab presses in the running app show the ring on the focused control.
  - Commit: not committed.
- [x] Task: Layouts at other sizes (1200x760, maximised), English and Thai
  - Verification Evidence: screenshots of Recorder, Recordings, Transcripts, Settings pages; fixed overlapping timer and clipped Pause label.
  - Commit: not committed.
- [x] Task: Embed the `.exe` icon (`build.rs`, `assets/icon/app.ico`)
  - Verification Evidence: icon extracted from the built exe shows the logo; version info shows the product name.
  - Commit: not committed.

- [x] Task: Audio capture engine and Recorder wiring (branch `feature/audio-capture`; spec `docs/superpowers/specs/2026-11-24-audio-capture-design.md`)
  - Built: session worker thread behind a `Backend` trait, WASAPI backend (microphone, whole-system loopback, per-process loopback of Zoom/Teams with whole-system fallback), WAV writer with periodic header flush, per-source clocks that pad gaps and rebase after Pause, real device lists, device choice saved by ID, Recorder screen driven by worker events (new Starting and Saved states, source status badges, timer from the worker, real error text).
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 105 passed, 4 ignored. The 4 ignored tests need audio hardware and passed here with `cargo test audio::wasapi_backend -- --ignored --test-threads=1` (microphone capture, whole-system loopback open, process-loopback open; Windows 11 build 26300).
  - Acceptance checks run on this machine, real exe, mouse clicks on Record / Pause / Resume / Stop, no Zoom or Teams running (so the system track was the whole-system fallback):
    - PASS both WAVs are valid mono 16-bit 48 kHz PCM files (parsed with Python `wave`; not listened to). A 4.41 s run gave 211752 frames in each file.
    - PASS track lengths: identical (0 ms) after Stop in the two stopped runs.
    - PASS a 3 s Pause is absent: 3 s record + 3 s pause + 3 s record gave 6.67 s in both files (the extra 0.67 s is the click delays); the timer held at 00:00:03 while paused.
    - PASS starting without Zoom or Teams shows the fallback badge (shown as "Capturing all system audio" in the first run, renamed "All system audio" afterwards and seen again on the Paused screenshot).
    - PARTIAL kill mid-recording: after killing the app at about 7 s both files open, but the headers say 5.01 s (last flush) while the data holds 6.99 s; the track lengths differ by 5 frames. Full length needs `recover_header`, which is not wired (see open task).
    - PASS closing the window during a recording finalizes: after 7 s both files had full headers (336784 frames each, equal) because the worker finalizes when the app drops it.
    - PASS the UI did not stall in any of the 5 recording runs (observed, not measured).
  - Not verified: capturing a real Zoom/Teams call, participant separation between tracks, quit/reopen of the meeting app, microphone unplug, process-loopback timestamps from a live process. These are the open task above; the roadmap item stays unticked.
  - Commit: branch `feature/audio-capture`, not pushed, no PR.

## Rules

- Every task must have a clear outcome.
- Split unrelated features.
- Do not mark tasks complete without verification.
