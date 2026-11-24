# Roadmap

## Current Milestone

- Goal: Native egui UI for every Stitch screen, then connect real backends.
- Status: M2 UI half Done; audio capture and transcription built (checked on this machine) but real-call and 2-minute-speech acceptance still open; Recordings and Transcripts read real folders; exports done (written next to the recording); playback done (mixed tracks, default device); SQLite index Not Started
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
- [ ] Transcription (OpenThai ASR on ONNX Runtime): built and checked on this machine (CER 4.7-5.3% on clips); clean-capture speech run and the licence gate open (see TASKS.md)
- [ ] Storage (SQLite index, WAV / transcript files): WAV and transcript files are real and the lists read them from disk; no SQLite index (not needed yet, see DECISIONS.md)
- [x] Exports (TXT, SRT, JSON): written next to the recording and shown in Explorer; no Save dialog
- [x] Playback of recordings (both tracks mixed, play / pause / seek, WASAPI default device): checked on this machine with the real exe and a loopback test; no speed, volume or device choice
- [x] Live transcript while recording (lines appear after each pause and the final transcript replaces them after Stop): checked on this machine with the real exe and model; no words for an unfinished sentence, a person speaking into the microphone not tried
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
