# Roadmap

## Current Milestone

- Goal: Native egui UI for every Stitch screen, then connect real backends.
- Exit Criteria: UI matches the design; `cargo fmt --check`, `cargo check`, `cargo clippy`, `cargo test` pass; backends wired and verified.
<!-- roadmap:begin -->
<!-- generated from TASKS.md by scripts/gen-roadmap.sh; do not edit between the markers -->
- Status: M2 - Core Features (In Progress)

## Milestones

### M1 - Foundation (Done)
- [x] Repository setup (Rust + eframe/egui 0.31, no WebView)
- [x] Architecture (screens / components / shell / theme / i18n / branding)
- [x] Build and test pipeline (fmt, clippy, test pass; 19 tests)

### M2 - Core Features (In Progress)
- [x] Implement all 10 Stitch screens and 3 modals as native egui UI
- [x] Restyle buttons, dropdowns, playback bar to the Stitch spec; bundle Inter / JetBrains Mono / Noto Sans Thai
- [x] Frameless window with custom title bar, drag, resize, rounded corners, inset window controls
- [x] Light-theme legibility (button text bug, darker semantic text colours, lighter font weights)
- [x] English / Thai UI language switching (`src/i18n.rs`)
- [x] App logo as window icon, sidebar logo and About logo (`src/branding.rs`)
- [x] Persist user preferences (theme, accent, font scale, language, general toggles)
- [x] App-wide shortcuts (Ctrl+Shift+R / P / A / C)
- [x] Transcription engine loading / ready / unavailable states
- [x] Keyboard focus ring
- [x] Layouts at other sizes (1200x760, maximised), English and Thai
- [x] Embed the `.exe` icon (`build.rs`, `assets/icon/app.ico`)
- [ ] Wire the UI to real backends (WASAPI capture, OpenThai ASR / ONNX, SQLite)
- [ ] Audio capture (Windows WASAPI, dual track)
- [ ] Transcription (OpenThai ASR on ONNX Runtime)
- [ ] Storage (SQLite index, WAV / transcript files)
- [ ] Exports (TXT, SRT, JSON)

### M3 - Stabilization (Not Started)
- [ ] Keyboard navigation inside tables and dropdown popups
- [ ] Error handling
- [ ] Performance review
- [ ] Security review
- [ ] Release validation (installer, release-build check)
<!-- roadmap:end -->

## Status Values

- Not Started
- In Progress
- Blocked
- Done

The status line and milestone lists above are generated from `TASKS.md` by `scripts/gen-roadmap.sh` (run automatically by the pre-commit hook). Edit `TASKS.md`, not the generated block. The Goal and Exit Criteria lines are hand-written; keep them in line with the current milestone.
