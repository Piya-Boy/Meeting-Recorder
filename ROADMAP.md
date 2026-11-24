# Roadmap

## Current Milestone

- Goal: Native egui UI for every Stitch screen, then connect real backends.
- Status: M2 UI half Done; audio capture built but real-call acceptance still open; ASR, SQLite, exports Not Started
- Exit Criteria: UI matches the design; `cargo fmt --check`, `cargo check`, `cargo clippy`, `cargo test` pass; backends wired and verified.

## Milestones

### M1 - Foundation (Done)
- [x] Repository setup (Rust + eframe/egui 0.31, no WebView)
- [x] Architecture (screens / components / shell / theme / i18n / branding)
- [x] Build and test pipeline (fmt, clippy, test pass; 19 tests)

### M2 - Core Features (In Progress)
- [x] UI: 10 screens, 3 modals, toasts, navigation, resizable frameless window
- [x] UI: dark and light themes, accent colours, font scaling, English / Thai language
- [x] UI: app logo, exe icon, shortcuts, keyboard focus, engine states
- [ ] Audio capture (Windows WASAPI, dual track): built and checked on this machine, real Zoom/Teams acceptance open (see TASKS.md)
- [ ] Transcription (OpenThai ASR on ONNX Runtime)
- [ ] Storage (SQLite index, WAV / transcript files)
- [ ] Exports (TXT, SRT, JSON)
- [x] Persist settings

### M3 - Stabilization (Not Started)
- [ ] Error handling
- [ ] Performance review
- [ ] Security review
- [ ] Release validation (installer, release-build check)

## Status Values

- Not Started
- In Progress
- Blocked
- Done

Keep this document synchronized with TASKS.md and actual implementation.
