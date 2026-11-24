# Task Tracker

Every task needs a `Milestone:` sub-bullet (`M<number> - <name>`). `ROADMAP.md` milestones are generated from this file by `scripts/gen-roadmap.sh`; edit tasks here, not in the generated block.

## In Progress

_None._

## Next

- [ ] Task: Wire the UI to real backends (WASAPI capture, OpenThai ASR / ONNX, SQLite)
  - Milestone: M2 - Core Features
  - Acceptance Criteria: Record / Pause / Stop produce dual-track WAV; live transcript from ASR; recordings and transcripts read from SQLite instead of mock data; UI talks to workers through channels.
  - Verification: Record a short session, reopen the app, find it in Recordings and Transcripts; `cargo test` covers the storage layer.
- [ ] Task: Audio capture (Windows WASAPI, dual track)
  - Milestone: M2 - Core Features
  - Acceptance Criteria: Record / Pause / Stop write a system-audio WAV and a microphone WAV; each source reports its own status.
- [ ] Task: Transcription (OpenThai ASR on ONNX Runtime)
  - Milestone: M2 - Core Features
  - Acceptance Criteria: Live partial and final Thai segments from a local model without blocking the UI.
- [ ] Task: Storage (SQLite index, WAV / transcript files)
  - Milestone: M2 - Core Features
  - Acceptance Criteria: Recordings and transcripts persist across restarts and Thai text is searchable.
- [ ] Task: Exports (TXT, SRT, JSON)
  - Milestone: M2 - Core Features
  - Acceptance Criteria: A saved transcript exports in each of the three formats.
- [ ] Task: Keyboard navigation inside tables and dropdown popups
  - Milestone: M3 - Stabilization
  - Acceptance Criteria: Arrow keys / Enter work in rows and popups; focus ring visible.
  - Verification: Real key presses with screenshots.
- [ ] Task: Error handling
  - Milestone: M3 - Stabilization
  - Acceptance Criteria: Device, capture, write and transcription failures show a useful message and keep recoverable data.
- [ ] Task: Performance review
  - Milestone: M3 - Stabilization
  - Acceptance Criteria: Recording and live transcription keep the UI responsive; findings and fixes recorded.
- [ ] Task: Security review
  - Milestone: M3 - Stabilization
  - Acceptance Criteria: Review of file paths, model loading and stored data completed; findings fixed or recorded.
- [ ] Task: Release validation (installer, release-build check)
  - Milestone: M3 - Stabilization
  - Acceptance Criteria: Release build runs on a clean Windows machine; installer installs and uninstalls cleanly.

## Blocked

_None._

## Completed

- [x] Task: Repository setup (Rust + eframe/egui 0.31, no WebView)
  - Milestone: M1 - Foundation
  - Verification Evidence: carried over from the previous ROADMAP.md; not re-verified in this change.
- [x] Task: Architecture (screens / components / shell / theme / i18n / branding)
  - Milestone: M1 - Foundation
  - Verification Evidence: carried over from the previous ROADMAP.md; not re-verified in this change.
- [x] Task: Build and test pipeline (fmt, clippy, test pass; 19 tests)
  - Milestone: M1 - Foundation
  - Verification Evidence: carried over from the previous ROADMAP.md; not re-verified in this change.
- [x] Task: Implement all 10 Stitch screens and 3 modals as native egui UI
  - Milestone: M2 - Core Features
  - Verification Evidence: every screen screenshotted from the running exe and compared with the Stitch render; real-click checks for navigation, delete modal, sidebar collapse, Appearance. See `UI_PROGRESS.md`.
  - Commit: not committed (repository has no commits; user has not asked for one).
- [x] Task: Restyle buttons, dropdowns, playback bar to the Stitch spec; bundle Inter / JetBrains Mono / Noto Sans Thai
  - Milestone: M2 - Core Features
  - Verification Evidence: screenshots vs Stitch; fonts and OFL licences in `assets/fonts`.
  - Commit: not committed.
- [x] Task: Frameless window with custom title bar, drag, resize, rounded corners, inset window controls
  - Milestone: M2 - Core Features
  - Verification Evidence: scripted real mouse tests (drag, maximise, restore, close, edge/corner resize, 1200x760 minimum).
  - Commit: not committed.
- [x] Task: Light-theme legibility (button text bug, darker semantic text colours, lighter font weights)
  - Milestone: M2 - Core Features
  - Verification Evidence: light-theme screenshots of all screens re-reviewed.
  - Commit: not committed.
- [x] Task: English / Thai UI language switching (`src/i18n.rs`)
  - Milestone: M2 - Core Features
  - Verification Evidence: all 10 screens and Delete/Help modals shot in Thai; live switch via the dropdown verified by clicking; unit test for lookup.
  - Commit: not committed.
- [x] Task: App logo as window icon, sidebar logo and About logo (`src/branding.rs`)
  - Milestone: M2 - Core Features
  - Verification Evidence: zoomed screenshots (dark and light); unit tests for transparent backdrop and icon size. The exe icon was verified separately (see the exe icon task).
  - Commit: not committed.
- [x] Task: Persist user preferences (theme, accent, font scale, language, general toggles)
  - Milestone: M2 - Core Features
  - Verification Evidence: unit tests (round trip, bad input); two real launches: language chosen in the dropdown was restored on relaunch; dev-flag runs neither read nor write the file.
  - Commit: not committed.
- [x] Task: App-wide shortcuts (Ctrl+Shift+R / P / A / C)
  - Milestone: M2 - Core Features
  - Verification Evidence: real key presses: Ctrl+Shift+R then P produced the Paused state; unit tests for the state transitions.
  - Commit: not committed.
- [x] Task: Transcription engine loading / ready / unavailable states
  - Milestone: M2 - Core Features
  - Verification Evidence: screenshots with `--engine=loading|error` on the Recorder and Settings > Transcription; unit tests for the mock load.
  - Commit: not committed.
- [x] Task: Keyboard focus ring
  - Milestone: M2 - Core Features
  - Verification Evidence: Tab presses in the running app show the ring on the focused control.
  - Commit: not committed.
- [x] Task: Layouts at other sizes (1200x760, maximised), English and Thai
  - Milestone: M2 - Core Features
  - Verification Evidence: screenshots of Recorder, Recordings, Transcripts, Settings pages; fixed overlapping timer and clipped Pause label.
  - Commit: not committed.
- [x] Task: Embed the `.exe` icon (`build.rs`, `assets/icon/app.ico`)
  - Milestone: M2 - Core Features
  - Verification Evidence: icon extracted from the built exe shows the logo; version info shows the product name.
  - Commit: not committed.

## Rules

- Every task must have a clear outcome.
- Every task needs a `Milestone:` sub-bullet.
- Split unrelated features.
- Do not mark tasks complete without verification.
