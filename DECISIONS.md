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

### 2026-11-24 - ROADMAP.md generated from TASKS.md

- Status: Accepted
- Context: ROADMAP.md and TASKS.md were edited by hand and drifted apart.
- Decision: TASKS.md is the single source of truth. Each task carries a `Milestone: M<number> - <name>` sub-bullet. `scripts/gen-roadmap.sh` (sh + awk) rewrites only the block between the `roadmap:begin` / `roadmap:end` markers in ROADMAP.md; `.githooks/pre-commit` runs it and stages the result. `.gitattributes` pins LF for `*.sh` and `.githooks/*` so `core.autocrlf=true` cannot break the shebang.
- Alternatives: A hook that only blocks commits until ROADMAP is updated by hand (not automatic); a reminder or CLAUDE.md rule (not enforced).
- Consequences: No new dependency. A task without a milestone tag blocks the commit. The hook is opt-in per clone (`git config core.hooksPath .githooks`). The script reads TASKS.md from the working tree, not the index, so a partly staged TASKS.md can produce a ROADMAP built from unstaged text. Goal and Exit Criteria in ROADMAP.md stay hand-written.
- References: `scripts/gen-roadmap.sh`, `scripts/test-gen-roadmap.sh`, `docs/superpowers/specs/2026-11-24-roadmap-autogen-design.md`
