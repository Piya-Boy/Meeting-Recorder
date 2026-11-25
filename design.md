# Meeting Recorder — UI Design Specification

> **Document:** `design.md`  
> **Product:** Meeting Recorder  
> **Target:** Windows 11 Desktop  
> **UI framework:** Rust + egui/eframe  
> **Design source:** Google Stitch export (removed from the tree; see section 14)  
> **Status:** Design baseline for HTML-to-egui migration

---

## 1. Purpose

This document defines the visual system, screen structure, interaction patterns, and implementation rules for rebuilding the Google Stitch HTML interface as a native Rust desktop UI using `egui`/`eframe`.

The Stitch HTML is the **visual and interaction reference**. It is not the runtime UI and must not be embedded in a browser view. Recreate its appearance and behavior with egui widgets and custom painting where necessary.

### Design priorities

1. Preserve the Stitch design as closely as practical.
2. Keep the interface calm, functional, and suitable for long meetings.
3. Make recording state and audio-source health immediately visible.
4. Keep live transcript text readable, including Thai.
5. Prefer reusable components and semantic design tokens.
6. Avoid decorative effects that are difficult to reproduce or maintain in egui.

## 2. Product UI Principles

- **Desktop-first:** Optimize for a resizable Windows desktop window.
- **Local-first:** Communicate local recording and storage behavior clearly.
- **State clarity:** Recording, paused, stopped, device error, and transcription states must be distinguishable.
- **Low distraction:** No unnecessary animation, gradients, glass effects, or decorative cards.
- **Consistent navigation:** Keep the main navigation visible across primary screens.
- **Predictable actions:** Destructive actions require confirmation.
- **Accessible contrast:** Text and status indicators must remain legible in both themes.

## 3. Design Tokens

### 3.1 Color palette

Use semantic color names in code. Do not scatter raw color literals throughout screen implementations.

#### Shared semantic colors

| Token | Hex | Usage |
|---|---|---|
| `brand_blue` | `#3B82F6` | Brand accent, focus, selected navigation |
| `primary` | `#2563EB` | Primary action |
| `success` | `#22C55E` | Recording active, healthy device, success |
| `warning` | `#F59E0B` | Device warning, partial/unfinalized state |
| `error` | `#EF4444` | Errors, failed device, destructive action |
| `on_success` | `#052E16` | Text on success surfaces |
| `on_warning` | `#451A03` | Text on warning surfaces |
| `on_error` | `#1C1C1C` | Text on error surfaces |

#### Dark theme (default)

| Token | Hex | Usage |
|---|---|---|
| `background` | `#111318` | Main application canvas |
| `sidebar` | `#171A21` | Main navigation rail |
| `surface` | `#1D222B` | Cards and panels |
| `surface_elevated` | `#242A35` | Elevated controls, hover surfaces, dialogs |
| `surface_variant` | `#242A35` | Secondary surfaces |
| `on_background` | `#F1F5F9` | Main foreground |
| `on_surface` | `#F1F5F9` | Primary panel text |
| `on_surface_variant` | `#94A3B8` | Secondary text |
| `outline` | `#2E3543` | Panel and control borders |
| `outline_variant` | `#1F242E` | Subtle separators |
| `primary_container` | `#1E3A5F` | Selected/contained primary surfaces |
| `on_primary_container` | `#DBEAFE` | Text on primary container |

#### Light theme

| Token | Hex | Usage |
|---|---|---|
| `background` | `#F8FAFC` | Main application canvas |
| `sidebar` | `#F1F5F9` | Main navigation rail |
| `surface` | `#FFFFFF` | Cards and panels |
| `surface_elevated` | `#E2E8F0` | Elevated controls |
| `surface_variant` | `#F1F5F9` | Secondary surfaces |
| `on_background` | `#0F172A` | Main foreground |
| `on_surface` | `#0F172A` | Primary panel text |
| `on_surface_variant` | `#475569` | Secondary text |
| `outline` | `#CBD5E1` | Panel and control borders |
| `outline_variant` | `#E2E8F0` | Subtle separators |
| `primary_container` | `#DBEAFE` | Selected/contained primary surfaces |
| `on_primary_container` | `#1E3A8A` | Text on primary container |

### 3.2 Color usage rules

- Dark mode is the initial default.
- Light mode must use the paired tokens above.
- Green is reserved for active/healthy/success states, not decoration.
- Amber is reserved for warnings and incomplete states.
- Red is reserved for errors and destructive actions.
- Do not communicate status through color alone; pair color with text or an icon.
- Use subtle 1px outlines instead of heavy shadows.

### 3.3 Typography

Preferred font families:

| Role | Font |
|---|---|
| General UI | Inter |
| Thai transcript fallback | Noto Sans Thai |
| Timers, timestamps, metrics, paths | JetBrains Mono |
| General fallback | Segoe UI, sans-serif |

| Token | Size | Weight | Line height | Usage |
|---|---:|---:|---:|---|
| `display_timer` | 36px | 600 | 44px | Main recording timer |
| `display_timer_compact` | 28px | 600 | 36px | Compact timer |
| `headline_lg` | 22px | 600 | 28px | Main page title |
| `headline_md` | 16px | 600 | 22px | Section title |
| `title_sm` | 14px | 600 | 20px | Panel title |
| `body_md` | 14px | 400 | 22px | Main UI text |
| `body_thai` | 15px | 400 | 26px | Thai transcript |
| `body_sm` | 13px | 400 | 18px | Supporting text |
| `label_lg` | 13px | 500 | 18px | Form labels |
| `label_sm` | 11px | 500 | 16px | Metadata labels |
| `mono_sm` | 12px | 400 | 18px | Technical values |

Use tabular numeric formatting for timers, durations, timestamps, and numeric columns where supported.

### 3.4 Spacing

Use a 4px base grid.

| Token | Value |
|---|---:|
| `space_1` | 4px |
| `space_2` | 8px |
| `space_3` | 12px |
| `space_4` | 16px |
| `space_6` | 24px |
| `space_8` | 32px |
| `page_margin` | 24px |
| `panel_padding` | 16px |

### 3.5 Shape and borders

| Element | Radius |
|---|---:|
| Meter segment / small tag | 4px |
| Button / input / dropdown | 6px |
| Card / panel | 8px |
| Large dialog / major container | 10px |

Use a 1px outline for cards, panels, tables, and controls. Avoid strong drop shadows.

### 3.6 Motion

- Hover and focus transitions: approximately 100ms.
- Dialog and panel entrance: approximately 150ms.
- Avoid bouncing, pulsing, or continuously animated decoration.
- Audio meters may update continuously, but updates must not cause layout movement.
- Respect reduced-motion/system preferences where practical.

## 4. Application Shell

### 4.1 Main window

- Target minimum usable size: **1200 × 760 px**.
- Window must resize without overlapping controls.
- Main navigation rail: **220px expanded**, **64px collapsed**.
- Main content page margin: **24px**.
- Keep the primary navigation consistent across the main screens.

### 4.2 Main navigation

The navigation shell should provide access to:

1. Recorder
2. Recordings
3. Transcripts
4. Settings

The current destination must have a clear selected state using the primary color/container treatment. Icons should use Lucide-style visual language and consistent stroke weight.

### 4.3 Settings shell

Settings pages use a secondary category rail approximately 220px wide and a content area containing the selected settings form.

Settings categories represented in the Stitch export:

- General
- Audio
- Transcription
- Storage
- Appearance
- Advanced

## 5. Screen Specifications

The Stitch archive contains ten screen designs. Rebuild each as a separate egui screen/view while sharing the application shell and reusable components.

### 5.1 Recorder — Split View

**Source:** `recorder_split_view/code.html`

Purpose: Main workspace for recording and monitoring the live transcript.

Layout:
- Main page title: `Recorder`.
- Recording/control area occupies approximately 35% of the content width.
- Live transcript panel occupies approximately 65%.
- Recording status and elapsed time must be prominent.
- Audio source status must be visible for both System Audio and Microphone.
- Transcript panel title: `Live transcript`.

Required UI states:
- Ready / idle.
- Recording.
- Paused.
- Stopping/finalizing.
- Audio device unavailable.
- Transcription engine unavailable.
- No transcript received yet.
- Live partial transcript and finalized transcript.

Behavior:
- Start, pause/resume, and stop actions must update the visible state.
- Stop must safely finalize and save the recording/session.
- Keep the transcript readable while new segments arrive.
- Do not block the egui update loop with audio or transcription work.

The source design also includes help/about content. Implement these as dialogs or secondary panels consistent with the visual system.

### 5.2 Recordings — Recording Library

**Source:** `recordings_recording_library/code.html`

Purpose: Browse and manage saved audio sessions.

Layout:
- Page title: `Recordings`.
- Recording list/table with clear metadata columns.
- Empty state: `No recordings yet`.
- Provide a clear path to open a recording or its transcript.

Behavior:
- Selecting a row opens the corresponding recording/session detail.
- Deleting a recording requires confirmation.
- Confirmation dialog title in the source: `Delete recording?`.
- Empty state must be intentional and useful, not a blank page.

### 5.3 Transcripts — Transcript Management

**Source:** `transcripts_transcript_management/code.html`

Purpose: Browse, search, and manage transcript documents.

Layout:
- Page title: `Transcripts`.
- List/table of available transcripts.
- Clear title, date/time, duration, and relevant status metadata.
- Provide an empty state and a selected-row state.

Behavior:
- Selecting a transcript opens Transcript Detail.
- Search/filter controls must visibly reflect the active query/filter.
- Destructive actions require confirmation.

### 5.4 Transcript Detail — Document Viewer

**Source:** `transcript_detail_document_viewer/code.html`

Purpose: Read and navigate a saved transcript.

Layout:
- Document-style reading area.
- Clear transcript title and session metadata.
- Readable line spacing and comfortable content width.
- Support Thai text without clipping or broken glyphs.

Behavior:
- Provide navigation back to the transcript list.
- Preserve transcript content and timestamps.
- Keep any export/copy actions visually distinct from destructive actions.

### 5.5 Settings — General

**Source:** `settings_general/code.html`

Purpose: General application preferences and keyboard shortcuts.

Requirements:
- Use the common Settings shell.
- Display General as the selected category.
- Include the keyboard-shortcut section represented in the source.
- Shortcut labels and key combinations must be easy to scan.

### 5.6 Settings — Audio

**Source:** `settings_audio/code.html`

Purpose: Configure audio inputs and capture behavior.

Requirements:
- Provide separate configuration/status for System Audio and Microphone.
- Show selected device names and unavailable-device errors.
- Provide input-level feedback where supported.
- Device selection must not interrupt an active recording without an explicit, safe workflow.
- Clearly communicate the selected source configuration.

### 5.7 Settings — Transcription

**Source:** `settings_transcription/code.html`

Purpose: Configure the transcription engine and language behavior.

Requirements:
- Show selected engine/provider and language configuration.
- Represent engine readiness, loading, and error states.
- Explain settings that affect live transcription.
- Do not imply that diarization is available in the initial scope.

### 5.8 Settings — Storage

**Source:** `settings_storage/code.html`

Purpose: Configure and inspect local storage.

Requirements:
- Show the recording/transcript storage location.
- Show storage usage when available.
- Provide a clear action for opening the storage location.
- Any move/delete/cleanup action must include confirmation and explain its impact.

### 5.9 Settings — Appearance

**Source:** `settings_appearance/code.html`

Purpose: Configure visual theme.

Requirements:
- Provide Dark, Light, and (if implemented) System theme selection.
- Dark is the initial default.
- Theme changes must update the entire application consistently.
- Persist the selected preference.

### 5.10 Settings — Advanced & About

**Source:** `settings_advanced/code.html`

Purpose: Expose advanced options and application information.

Requirements:
- Separate Advanced and About content clearly.
- Show app version/build information when available.
- Avoid exposing debug-only controls in normal user workflows.
- Any advanced action that may affect recordings or data must explain the consequence.

## 6. Reusable UI Components

Implement shared components before duplicating screen-specific UI.

| Component | Responsibility |
|---|---|
| `AppShell` | Main window layout and navigation |
| `Sidebar` | Primary navigation rail |
| `SettingsSidebar` | Settings category navigation |
| `PageHeader` | Page title and optional actions |
| `Panel` | Surface, border, padding, and title |
| `PrimaryButton` | Main action |
| `SecondaryButton` | Secondary action |
| `IconButton` | Compact icon-only action with tooltip |
| `StatusBadge` | Recording/device/transcription status |
| `AudioSourceCard` | Source name, device, status, and meter |
| `AudioLevelMeter` | Input-level visualization |
| `RecordingTimer` | Monospaced elapsed time |
| `TranscriptSegment` | Partial/final transcript segment |
| `DataTable` | Recording/transcript rows |
| `EmptyState` | Empty list or unavailable content |
| `ConfirmDialog` | Destructive-action confirmation |
| `SettingsRow` | Label, description, and setting control |

All reusable components must use the shared theme tokens.

## 7. egui Implementation Guidance

### 7.1 General mapping

| Stitch HTML/CSS concept | egui implementation direction |
|---|---|
| `div` / container | `ui.vertical`, `ui.horizontal`, `ui.scope`, `Frame` |
| Flex row | `ui.horizontal` |
| Flex column | `ui.vertical` |
| Button | `ui.button` or custom button helper |
| Text input | `ui.text_edit_singleline` |
| Multiline text | `ui.text_edit_multiline` |
| Select/dropdown | `egui::ComboBox` |
| Toggle | `egui::Checkbox` or custom toggle |
| Grid/table | `egui::Grid`, table crate, or custom row layout |
| Card/panel | `egui::Frame` |
| Modal | `egui::Window` or custom modal overlay |
| Border/radius/fill | `Frame`, `Stroke`, `CornerRadius`, `Color32` |
| Icon | SVG/raster texture or icon font, with consistent sizing |
| CSS spacing | `ui.add_space` and layout spacing |
| Scroll container | `egui::ScrollArea` |

Exact APIs may vary with the pinned egui version. Check the project's dependency version before copying API examples.

### 7.2 Custom painting

Use custom painting only where standard egui widgets cannot match the reference adequately, such as:
- Audio level meters.
- Recording waveform or timeline indicators.
- Fine-grained status indicators.
- Special separators or selection treatments.

Do not build every control from raw painter primitives. Prefer standard egui widgets for keyboard navigation and accessibility.

### 7.3 Fonts and Thai support

- Bundle or load Inter, Noto Sans Thai, and JetBrains Mono where licensing permits.
- Configure egui font definitions with Thai glyph coverage.
- Test combining marks, tone marks, vowel placement, and mixed Thai/English lines.
- Do not assume the default font has complete Thai glyph coverage.
- Use a consistent fallback strategy for missing glyphs.

### 7.4 Layout and resizing

- Use available width/height rather than fixed absolute positions.
- Preserve the 35/65 recorder split at normal desktop widths.
- At narrower widths, allow panels to stack or switch layout deliberately.
- Keep essential recording controls visible at all supported sizes.
- Avoid clipping the transcript or settings controls.

### 7.5 UI/backend separation

UI code must not perform blocking audio, file, database, or transcription operations.

Use channels/events to communicate between UI and background workers:
- UI sends commands.
- Background workers perform work.
- Workers send state/events/results.
- UI consumes events and updates application state.

The UI should remain responsive while recording, saving, or transcribing.

## 8. Interaction and State Rules

### Recording lifecycle

The UI must represent these states explicitly:

```text
Idle -> Starting -> Recording -> Paused -> Recording
                         |
                         v
                      Stopping -> Finalizing -> Saved
```

Error states may occur during device startup, capture, writing, or transcription. Show a useful error message and preserve recoverable recording data where possible.

### Audio source behavior

The initial product requirement is to support both:
- System Audio
- Microphone

Keep the two source statuses independently visible. Do not assume both devices are available. If one source fails, communicate whether recording can continue with the other source.

### Transcript behavior

- Distinguish partial text from finalized text.
- Keep chronological ordering.
- Preserve timestamps where available.
- Avoid excessive auto-scrolling that prevents users from reading older content.
- Provide a clear indication when transcription is paused, unavailable, or catching up.

### Destructive actions

- Confirm deletion of recordings/transcripts.
- Name the affected item in the confirmation.
- Provide Cancel as the safe default/focused action where practical.
- Do not silently delete source audio when deleting a transcript unless the product explicitly defines that behavior.

## 9. Accessibility and Usability

- Use text labels or tooltips for icon-only buttons.
- Ensure keyboard focus is visible.
- Maintain sufficient contrast in both themes.
- Avoid color-only status communication.
- Make controls large enough to click reliably.
- Support keyboard navigation for navigation, forms, tables, and dialogs.
- Ensure Thai text is not clipped or rendered with missing glyphs.
- Keep critical recording controls easy to locate.

## 10. Assets and Icons

The Stitch HTML may reference remote fonts, icon libraries, or remote assets. For the Rust desktop build:

1. Inventory all external assets.
2. Identify which assets are actually used.
3. Prefer local bundled assets for reliable offline operation.
4. Verify font and icon licenses before bundling.
5. Avoid runtime dependence on remote CDNs.
6. Use a consistent Lucide icon style; do not mix unrelated icon families.

## 11. Design-to-Implementation Workflow

For each screen:

1. Open the matching Stitch HTML file in a browser.
2. Record its layout, content hierarchy, colors, spacing, typography, and interaction states.
3. Implement the screen using shared egui components.
4. Compare the egui result against the HTML reference at the same window dimensions.
5. Fix layout and typography differences before adding backend behavior.
6. Test resizing, keyboard focus, empty states, and error states.
7. Record any intentional deviation from the source design.

### Screen migration checklist

- [x] Recorder — Split View
- [x] Recordings — Recording Library
- [x] Transcripts — Transcript Management
- [x] Transcript Detail — Document Viewer
- [x] Settings — General
- [x] Settings — Audio
- [x] Settings — Transcription
- [x] Settings — Storage
- [x] Settings — Appearance
- [x] Settings — Advanced & About

## 12. Out of Scope for the Initial UI Migration

The following are not required to complete the first visual migration:
- Speaker diarization.
- Complex waveform editing.
- Cloud synchronization.
- Multi-user collaboration.
- Decorative animation systems.
- Recreating the HTML runtime or embedding a web browser.

These may be considered later as separate product features.

## 13. Acceptance Criteria

The UI migration is considered complete when:

- [x] All ten Stitch screens have corresponding egui views.
- [x] Main navigation and Settings navigation work.
- [x] Dark theme matches the supplied design tokens.
- [x] Light theme is implemented or explicitly tracked as a later task.
- [x] Typography, including Thai transcript text, renders correctly.
- [x] Recorder split layout follows the 35/65 reference at target dimensions.
- [x] Buttons, inputs, dropdowns, dialogs, and tables have functional interactions. (UI-level; backend actions show a toast)
- [x] Empty, loading, success, warning, and error states are represented. (engine loading / ready / unavailable included)
- [x] Window resizing does not cause important controls to overlap or disappear. (screenshot-verified at 1200x760, 1280x800 and maximised; recorder timer switches to the compact size when narrow)
- [ ] UI work does not block on audio, database, or transcription tasks. (audio capture and transcription run on worker threads behind command and event channels; storage reads folders on a background scan)
- [x] No runtime dependency on the original Stitch HTML is required.

## 14. Source Files

The original Stitch export (`stitch_meeting_recorder_desktop_ui`: design system markdown plus ten `code.html` screens) is no longer kept in the working tree. It remains in git history at commit `696e6b8` (`git show 696e6b8:stitch_meeting_recorder_desktop_ui/meeting_recorder_design_system.md`).

This specification was derived from that design system and the ten HTML screen exports. The egui implementation is now the source of truth for screen behavior.

## 15. Implementation Status and Intentional Deviations

Status as of 2026-11-24 (details: `UI_PROGRESS.md`, `TASKS.md`, `DECISIONS.md`).

### Done
- Ten screens, Help / About / Delete modals, toasts, shell, Settings rail, dark and light themes, accent colours, font scaling, English / Thai UI language, app logo (window icon, sidebar, About, `.exe` icon).
- Preferences persist across restarts (theme, accent, font scale, language, general toggles).
- Shortcuts work while the app window has focus: Ctrl+Shift+R start/stop, P pause/resume, A auto-scroll, C copy transcript.
- Keyboard focus ring after Tab; Space / Enter activate the focused control.
- Transcription engine states (loading / ready / unavailable) on the Recorder and in Settings > Transcription.
- Layouts checked at 1200x760, 1280x800 and maximised, in English and Thai.
- Audio capture (details: `docs/superpowers/specs/2026-11-24-audio-capture-design.md`): the Recorder starts, pauses, resumes and stops a real session and writes `system.wav` and `mic.wav` (mono 16-bit PCM) into `<destination>\<YYYY-MM-DD_HHMM>_<name>\`. Device lists come from Windows; the chosen devices are saved by ID and shared by Settings > Audio and the Recorder. New states: Starting (button pressed, session not up yet) and Saved (files closed; the folder path is shown and a toast names it); Stopping and Finalizing show as Processing. Each source shows a status: Ready, Connected (microphone), "All system audio" (system source, green), "Using Windows default" (system source, warning colour: the chosen output device cannot be opened), "Device lost", "Could not start", Off. A small line under the System Audio device list says that everything played on that device is recorded and asks for the device the meeting is heard on. Error text on the banner is the worker's message. Dev runs without `--audio` keep the old mock behavior.
- Transcription (details: `docs/superpowers/specs/2026-11-24-asr-transcription-design.md`): after Stop the Recorder transcribes `mic.wav` and `system.wav` with OpenThai ASR (ONNX Runtime, on this computer) and writes `transcript.json` and `transcript.txt` next to them. The Transcript panel has these states: empty ("no transcript yet" with a Transcribe button on a saved recording), Waiting (model not installed: a banner and an "Open Transcription settings" button), Transcribing (progress "mm:ss / mm:ss" and Cancel), done (lines with a time stamp and a You / Others chip, footer "N lines"), failed (error banner). Settings > Transcription shows the real model state and has Download / Cancel / Delete, precision (int8, fp32) and CPU threads (4, 6, 8), all saved. Licences: Settings > Advanced > Open-source licenses shows `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.txt`; the About modal points there.
- Recordings and Transcripts from real data (details: `docs/superpowers/specs/2026-11-24-library-real-data-design.md`): Recordings lists the session folders under the Recorder's destination (name, date, duration, size, status Transcribed / Processing n% / Audio only) with working search, date filter and sort; buttons: Open transcript, Transcribe (audio-only rows), Open folder, Delete (modal; permanent; only the files the app wrote). Transcripts lists the transcribed ones with a line count and the first line; search covers the title and every line. Transcript Detail shows the real lines read-only with You / Others chips; Copy all equals `transcript.txt`. The lists refresh when the screen opens, after a save, after a transcription and after a delete. Dev-flag runs without `--audio` keep the sample data (and the "Preview empty state" demo button).
- Export (details: `docs/superpowers/specs/2026-11-25-transcript-export-design.md`): the JSON, SRT and TXT buttons on the Transcripts rows and the detail page write the file next to the recording (`transcript.json` and `transcript.txt` are the ones the transcription wrote and are never overwritten; `transcript.srt` is rewritten each time) and show it selected in Explorer, with a notice. Sample rows keep the "not wired" toast.
- Playback (details: `docs/superpowers/specs/2026-11-25-playback-design.md`): the play button on a real Recordings row plays `mic.wav` and `system.wav` mixed through WASAPI on the Windows default output device. The bottom bar shows the title, "Track: System + Mic Mix", elapsed and total time, a seek bar (click, or drag and release) and a close button; pause, resume, seek and play to the end work, deleting the playing recording stops the player first, and playing is refused (and playback stopped) while a recording is in progress. The speed chip and the volume icon are not drawn for real recordings.
- `cargo fmt --check`, `cargo clippy --all-targets` and `cargo test` (410 tests, 7 more need hardware, the model or clips) pass.

### Intentional deviations from this document
- **Icons:** Phosphor (`egui-phosphor`) instead of Lucide (section 10).
- **Font weights:** one step lighter than the table in 3.3 (egui renders text heavier than a browser): headings 500 instead of 600, buttons 400 instead of 500, mono bold 500 instead of 700.
- **Light theme text:** `on_background` / `on_surface` are `#1E293B` instead of `#0F172A`; text-only variants `success_text` `#15803D`, `warning_text` `#B45309`, `error_text` `#B91C1C`, `link_text` `#1D4ED8` replace the bright semantic colours for text on light surfaces (3.2: legibility). Dark theme is unchanged.
- **Window:** frameless with a custom title bar and window controls; no Windows 11 snap-layout flyout.
- **Language:** English / Thai dictionary (`src/i18n.rs`); missing entries fall back to English.
- **Timer:** 28px compact timer below ~290px panel width (the spec's `display_timer_compact`).
- **Live transcript:** the model reads a whole segment, so the sentence being spoken is shown as grey italic preview text that is re-read every 2 s and replaced by the final (white) line when its segment is cut (7.x, 9: partial text is distinguished from final text by colour and italics; there is no "Partial" badge). The panel says "Listening..." until the first text. Settings > Transcription "Partial transcription" turns the live text on or off and "Streaming chunk window" is how often new audio is read (`DECISIONS.md`).
- **Transcription language and word fixes:** Settings > Transcription "Language" is a select (Thai, Thai + English) instead of a fixed "Thai (TH)" pill, and a "Word fixes" box below the live settings holds the user's rules (one a line, `wrong = right`) with a count of fixes and of lines that are not rules. The Recorder header pill still says "Thai (TH)".
- **Update check:** Settings > General has a "Check for updates" switch (default on). When a newer version is found a toast says so and the sidebar footer shows "Update available: vN.N.N" above the version (a link to the release page; the collapsed sidebar shows nothing). Settings > Advanced > About has a row under the version line with "Check for updates", "Open download page" (primary button, only when a version was found) and a status line ("Checking for updates...", "You have the latest version.", "Update available: vN.N.N", "Could not check for updates.").

### Not done (needs a backend, outside the UI migration scope)
- SQLite index (not needed yet) and a Save dialog for exports (they go next to the recording). Playback has no speed, volume, output-device choice or single-track listening; the detail page cannot edit text; a folder changed in Explorer shows only after the screen is opened again. The Recorder's transcript is real: lines appear while recording as a preview and the final transcript replaces them after Stop; the sentence being spoken is shown as grey preview text that can still change (see the Live transcript deviation above).
- Transcription is not accepted on a clean 2-minute capture of real speech (`TASKS.md`).
- Audio capture is not accepted yet on a real call with another person speaking: participant separation and unplugging a device are unverified (`TASKS.md`). After a crash the WAV headers cover only the audio up to the last 1 s flush; `recover_header` is not wired in. The recordings folder cannot be changed (`%USERPROFILE%\Documents\MeetingRecorder\Recordings`).
- Channel-based UI / backend separation (7.5): done for audio capture and transcription (worker threads, command and event channels); storage has no worker yet.
- Keyboard navigation inside tables and dropdown popups was not separately verified (Tab order and focus ring on buttons, toggles, nav items were).
- Thai stacked marks: egui does no text shaping, so `src/thai_marks.rs` moves a tone mark (or thanthakhat) up after layout when an upper vowel or the ring of sara am is under it. Thai transcript paragraphs use a line height of 1.5 font sizes. Other text keeps egui's line height, so only the first line of a wrapped text is raised there: a mark raised in a later line would read as a sara u of the line above. Marks are not moved sideways for ป, ฝ and ฟ.
- Reduced-motion preference: the UI has no animation beyond egui defaults.
