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
- Mock data: exports and the SQLite index do not exist yet. Audio capture, transcription and the Recordings / Transcripts / Detail lists are real (see `TASKS.md`); they are used when the app starts normally or with `--audio`, and other dev-flag runs keep the sample data.
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
- Engine: in preview runs a mock `Loading` (1.5 s) to `Ready`, or forced `Error`; in a normal run the state comes from the transcription worker (model missing, downloading, loading, ready, error). Shown as a badge on the Recorder, a notice when unavailable, and a pill in Settings > Transcription.
- Sizes: checked at 1200x760, 1280x800, maximised, English and Thai. Fixed: timer overlapping the sample-rate label (compact 28px timer when narrow), clipped Thai "Pause" label (now "พัก").
- Not verified: arrow-key navigation inside tables and dropdown popups.

## Transcription on the Recorder and in Settings (2026-11-24)

- Recorder panel with real data (only in a run that has the audio worker; preview runs keep the mock transcript): "Transcript" header with the engine pill, search, auto-scroll, copy; Transcribe button on a saved recording; Waiting banner (model missing) with "Open Transcription settings"; "Transcribing..." with progress and Cancel; lines with time stamp and a You / Others chip; "N lines" footer; failure banner.
- Settings > Transcription: ASR engine pill from the real model state, model folder row with Open folder, Precision (int8 / fp32), Model row with Download / Cancel / Delete, progress bar with MB, status text, CPU threads (4, 6, 8). Precision and threads are saved.
- Dev flag `--engine=missing|downloading` joins `loading|ready|error`.
- Checked with screenshots of the real exe (English): empty, Waiting, downloading (838 MB / 946 MB), ready, Transcribing, done with lines, recording a second session while the first transcribes. Not screenshotted: the Thai running state.
- Known: the "Recording saved to" toast covers the footer Cancel button for a few seconds.

## Recordings, Transcripts and Detail with real data (2026-11-24)

- Run with `--audio` (or normally) the three screens list the real session folders; any other dev flag keeps the sample data.
- Recordings: real rows (name, folder, date, duration, size, status pill), search / date filter / sort work, Open folder icon and a Play button, a bottom player for the loaded recording, Transcribe starts a job and the pill shows "Processing n%", Delete opens the existing modal. "Loading recordings..." until the first scan arrives.
- Transcripts: title, date, duration, "Lines" column, first line as preview; search placeholder without "(SQLite FTS)"; Copy gives the real text.
- Detail: date, duration, "N lines"; rows with time stamp, You / Others chip, selectable text; no edit hint (read-only).
- Checked with screenshots of the real exe in English: list, delete modal, processing, transcribed, Transcripts list, detail (aligned chip column). Not screenshotted: Thai, light theme, long transcript.

## Export buttons with real files (2026-11-25)

- Transcripts rows and the detail top bar: JSON, SRT and TXT now write the file next to the recording and open Explorer with it selected; the toast says "Exported. The file is in the recording's folder." (or "Could not export the transcript."). Sample rows (dev flags without `--audio`) still say "not wired".
- Checked with screenshots of the real exe in English (detail page with the toast). Not screenshotted: Thai, light theme.

## Playback with real recordings (2026-11-25)

- Recordings live rows: a Play button before Open folder (it becomes a filled Pause button for the recording that is playing). The bottom bar appears for the loaded recording: play/pause, title, "Track: System + Mic Mix", elapsed time, a seek bar (a click seeks at once; a drag shows the new place and seeks on release), total time, and a close (X) button. The speed chip and the volume icon are drawn only in the sample (preview) mode.
- New strings (English and Thai): "Play", "Close player", "Stop recording before playing.", "Playback stopped because a recording started.", "Could not play the recording.".
- Checked with screenshots of the real exe in English: bar while playing, paused, after a seek, at the end, the delete modal while playing, and the two toasts. Not screenshotted: Thai, light theme.

## Live transcript while recording (2026-11-25)

- Recorder transcript panel: while a recording runs with the model installed and "Partial transcription" on, the panel says "Listening..." and then shows lines in time order (time, "You" or "Others", text); the final transcript replaces them after Stop. With the setting off the panel keeps its old text until Stop.
- Settings > Transcription: "Partial transcription" now reads "Show transcript lines while recording. Each line appears after a pause." and "Streaming chunk window" reads "How often new audio is read for the live lines."; the note under them now says "Live lines are a preview. The final transcript is made after you stop and may differ a little." New strings (English and Thai): "Listening...", "Live transcript stopped." and the three above.
- Checked with screenshots of the real exe in English: "Listening...", lines at 59 s and at 2:01, the final transcript after Stop, the setting off. Not screenshotted: Thai, light theme.

## Preview text while recording (2026-11-25)

- Recorder transcript panel: the sentence being spoken shows as grey italic text (with its time and the "You" / "Others" badge) below the final lines, one row per track, and is replaced by the final white line when its segment ends. "Listening..." shows only until the first text.
- Settings > Transcription: "Partial transcription" now reads "Show transcript text while recording. Grey text may still change; a line is final after a pause." (English and Thai).
- Checked with screenshots of the real exe in English: the first grey text at 00:00:06, more words at 00:00:09, a final line plus a new grey row at 00:00:18, four final lines at 00:00:58. Not screenshotted: two tracks speaking at once, Thai, light theme.

## Language and word fixes (2026-11-25)

- Settings > Transcription: "Language" is now a select ("Thai", "Thai + English") with the description "Thai writes every word in Thai letters; Thai + English writes English words in Latin letters." Below the live settings, "Word fixes": a description, a multi-line box (hint "มอล, หมอก = มอก.") and a line "N fixes", plus "· M lines not understood (use wrong = right)" in the warning colour when a line is not a rule. English and Thai strings; the old "Primary recognition language." string is gone.
- Checked with screenshots of the real exe in English: the select showing "Thai + English" read from `settings.ini`, and Latin words in the live and final Recorder text. Not screenshotted: the word-fix box with typed rules, Thai, light theme.

## Update check (2026-11-25)

- Settings > General: a "Check for updates" row (switch, default on) with the description "Asks GitHub once a day whether a newer version exists. No recording or personal data is sent."
- Settings > Advanced > About: the version line comes from `Cargo.toml`; a row below it holds "Check for updates", "Open download page" and a status line. The first try put these buttons in the header row and cut the version label, so they moved.
- Sidebar footer: "Update available: vN.N.N" in the accent colour above the version when a newer version was found; the About modal and the footer show the version from `Cargo.toml`. English and Thai strings.
- Checked with screenshots of the real exe in English (dark theme): General, Advanced (newer version found, and up to date) and the Recorder with the toast and the sidebar link. Not screenshotted: Thai, light theme, the collapsed sidebar.

## 2026-11-25 - The real app no longer shows design sample data

- Settings > Storage: one read-only field, the recordings folder, and a line about what each recording folder holds. The transcript folder, automatic saving, database, cache and Browse rows are gone: none of them existed.
- Settings > Advanced: only the About card. The log folder, model cache and performance rows are gone.
- Recorder: no free-space figure and no state switcher outside the preview; the empty recording name shows "recording" as a hint; the language pill follows Settings > Transcription.
- Where this differs from the Stitch design, the app follows what it can really do.

