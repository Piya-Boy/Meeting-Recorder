# UI Implementation Progress — Meeting Recorder

Stack: Rust + egui/eframe 0.31 (native, no WebView). UI only — no audio/ASR backend.

## Overall Status
- Screens: 10 / 10 implemented
- Modals: 3 / 3 implemented (Help, About, Delete Recording)
- Current Target: none — Definition of Done met (see "Known limitations")

## Screen Checklist
- [x] 1. Recorder — Split View (idle / starting / recording / paused / processing / saved / error states)
- [x] 2. Recordings — Recording Library (search, date filter, sort, inline player, delete, empty state)
- [x] 3. Transcripts — Transcript Management (search, open, copy, TXT/SRT/JSON)
- [x] 4. Transcript Detail — Document Viewer (breadcrumb, copy all, export, editable paragraphs)
- [x] 5. Settings — General & Shortcuts
- [x] 6. Settings — Audio
- [x] 7. Settings — Transcription
- [x] 8. Settings — Storage
- [x] 9. Settings — Appearance (theme cards, accent, font scaling — live)
- [x] 10. Settings — Advanced & About

## Modals & Overlays Checklist
- [x] Help Modal (keyboard shortcuts)
- [x] About Modal (version / architecture)
- [x] Delete Recording Confirmation Modal (really deletes; bypassed when "confirm before deleting" is off)
- [x] Toasts for actions whose backend does not exist yet

## Verification (latest run)
| Check | Result |
|---|---|
| `cargo fmt --check` | pass |
| `cargo check` | pass, 0 warnings |
| `cargo clippy --all-targets` | pass, 0 warnings |
| `cargo test` | 19 passed (screen names, dev flags, theme accent, table columns, transcript text, i18n lookup, logo, settings store, recorder transitions, mock engine load) |
| Visual review | every screen screenshotted from the running exe and compared with a headless render of the Stitch HTML; dark + light checked |
| Interaction (real mouse clicks) | sidebar nav, Transcripts → Open → Detail, Recordings delete modal → Delete, sidebar collapse, Appearance accent + theme card |

Dev flags for review: `--screen=<name> --theme=light|dark --state=idle|recording|paused|processing|error --modal=help|about|delete --lang=en|th --engine=loading|ready|error`. Runs with any `--` flag do not read or write saved preferences.

## Problems found and fixed
- egui 0.31 API drift in the draft (82 errors) → ported (CornerRadius u8, Margin i8, `Frame::NONE`, `StrokeKind`, `show_inside`).
- `desired_size.x >= 0.0` panic in the Recorder layout → panels plus `.max(0.0)` clamps.
- Frames inside centred layouts stretched full height → wrapped in `ui.horizontal`.
- `strong()` only recoloured text → real bold faces (Segoe UI Bold / Consolas Bold / Leelawadee UI Bold) registered as `Bold` / `MonoBold` families.
- Premultiplied-alpha tints too bright → `from_rgba_unmultiplied`.
- Modals were hand-rolled windows → `egui::Modal` (scrim, Esc, click-outside).
- Boxed panel stroke produced a near-black seam between sidebar/topbar in light theme → stroke removed, separator uses `outline`.
- Buttons now 36px (compact 26px), medium-weight label, `hover:opacity-90` like Stitch (no blue hover border); Recorder transport row uses the shared `block_button`.
- OS title bar replaced by the app's own: frameless window (`with_decorations(false)`), drag + double-click-maximise on the top bar / sidebar brand row, min / max-restore / close buttons, edge + corner resize via `BeginResize`. Verified with real mouse input (drag, maximise, restore, close, resize, 1200×760 minimum).
- Frameless window corners squared off on Windows 11 → DWM round-corner preference requested at startup.
- Light theme: button label was hard-coded white (invisible on light buttons) → theme colour; bright success / error / link colours were ~2:1 on white → darker text-only variants (`success_text`, `error_text`, `link_text`) in light, unchanged in dark.
- Text read too heavy / dark → weights lowered one step, light-theme body text `#1E293B`.
- Window controls touched the window edge and the theme button → rounded 36x32 buttons inset 8px.
- Audio settings: Test buttons were not flush with the card edge → right-aligned, dropdown fills the rest; title-only settings rows vertically centred.
- Dead code / unused imports / identical `if` blocks removed (badge.rs deleted).

## Known limitations (honest)
- Superseded: the early `Segoe UI Bold / Consolas Bold / Leelawadee UI Bold` fix above was replaced by the bundled fonts below.
- Fonts: Inter (400/500), JetBrains Mono (400/500) and Noto Sans Thai (400/500) are bundled in `assets/fonts` (SIL OFL, licences alongside). PUA glyphs were stripped from Inter/JetBrains cmaps so they do not shadow Phosphor icons. egui has no variable weights, so each weight is its own family (`medium_family` = 400, `bold_family` = 500, `mono_bold_family` = 500). They are one step lighter than Stitch's CSS weights because egui renders text heavier than a browser.
- Icons: Phosphor (egui-phosphor) substitutes Lucide.
- Buttons that need a backend (hardware test, folder picker, optimize DB, export diagnostics, …) show a "not wired up yet (UI preview)" toast.
- Frameless: Windows 11 snap-layout flyout on the maximise button is not available (custom button); drag-to-edge snapping and Win+Arrows still work. Window is resizable (min 1200×760); layouts use flexible columns, but only 1280×800 was screenshot-verified.
- Mock data: ASR, SQLite and exports do not exist yet. Audio capture is real (see `TASKS.md`); the Recorder uses it when started normally or with `--audio`, and keeps the old mock behavior for other dev-flag runs.
- Nothing is committed: repo has no commits or remote and the user has not asked for one.

## Language switching (English / Thai)

- `src/i18n.rs`: `tr(&str)` maps English UI strings to Thai through a dictionary; unknown strings (device names, paths, mock transcript text) pass through unchanged. The active language is a process-global set each frame from Settings > General > "Application language".
- Translation happens inside shared components (buttons, dropdown labels, table headers, settings kit, toast, search hint) so most call sites need no change; dynamic strings are composed from translated parts (`not_wired`, `export_not_wired`).
- Dev flag: `--lang=en|th`.
- Verified: all 10 screens plus Delete/Help modals screenshotted in Thai; live switch through the dropdown verified by clicking.
- Limitations: mock recording/transcript content stays as authored; adding a string means adding a dictionary entry (missing entries show English).

## App logo

- `assets/icon/Icon.png` (RGB, black backdrop) is embedded; `src/branding.rs` flood-fills the backdrop to transparency at startup and resizes with premultiplied alpha.
- Used as: window/taskbar icon (`ViewportBuilder::with_icon`), sidebar brand logo, About card logo. Falls back to the old blue microphone tile if decoding fails.
- The `.exe` icon is embedded by `build.rs` (`winresource`, `assets/icon/app.ico` with 16/32/48/256 px). If the resource compiler is missing the build continues with a warning and no exe icon.

## Persistence, shortcuts, focus, engine states, sizes

- Preferences: `src/settings_store.rs` writes `%APPDATA%\MeetingRecorder\settings.ini` (key=value, atomic write, unknown keys ignored) when theme, accent, font scale, language or a general toggle changes. Verified with two real launches.
- Shortcuts: Ctrl+Shift+R start/stop, P pause/resume, A auto-scroll, C copy transcript (disabled while a modal is open). Verified with real key presses.
- Focus: after Tab a blue ring outlines the focused control (cleared on mouse press); the sidebar logo is only focusable when it expands a collapsed sidebar.
- Engine: mock `Loading` (1.5 s) to `Ready`, or forced `Error`; shown as a badge on the Recorder, a notice when unavailable, and a pill in Settings > Transcription.
- Sizes: checked at 1200x760, 1280x800, maximised, English and Thai. Fixed: timer overlapping the sample-rate label (compact 28px timer when narrow), clipped Thai "Pause" label (now "พัก").
- Not verified: arrow-key navigation inside tables and dropdown popups.

## Transcription on the Recorder and in Settings (2026-11-24)

- Recorder panel with real data (only in a run that has the audio worker; preview runs keep the mock transcript): "Transcript" header with the engine pill, search, auto-scroll, copy; Transcribe button on a saved recording; Waiting banner (model missing) with "Open Transcription settings"; "Transcribing..." with progress and Cancel; lines with time stamp and a You / Others chip; "N lines" footer; failure banner.
- Settings > Transcription: ASR engine pill from the real model state, model folder row with Open folder, Precision (int8 / fp32), Model row with Download / Cancel / Delete, progress bar with MB, status text, CPU threads (4, 6, 8). Precision and threads are saved.
- Dev flag `--engine=missing|downloading` joins `loading|ready|error`.
- Checked with screenshots of the real exe (English): empty, Waiting, downloading (838 MB / 946 MB), ready, Transcribing, done with lines, recording a second session while the first transcribes. Not screenshotted: the Thai running state.
- Known: the "Recording saved to" toast covers the footer Cancel button for a few seconds.
