# Meeting Recorder

A Windows desktop app that records a meeting's system audio and microphone as two separate tracks and transcribes Thai speech locally, so recordings never have to leave the machine.

The app is a native Rust program built with [egui / eframe](https://github.com/emilk/egui). It does not use a WebView or any browser runtime.

## Status

The user interface is finished. The recording, transcription and storage backends are not built yet.

| Area | State |
|---|---|
| Ten screens, three modals, toasts, navigation | Done |
| Dark and light themes, accent colours, font scaling | Done |
| English and Thai interface language | Done |
| Saved preferences, keyboard shortcuts, focus ring | Done |
| App logo (window, sidebar, About, `.exe` icon) | Done |
| Audio capture (Windows WASAPI, dual track) | Built; real Zoom / Teams call check still open |
| Transcription (OpenThai ASR on ONNX Runtime) | Built; lines appear while recording, the final transcript is made after Stop |
| Storage (SQLite index, WAV and transcript files) | WAV and transcript files and the library lists are real; no SQLite index |
| Exports (TXT, SRT, JSON) | Done: written next to the recording and shown in Explorer |

The Recorder captures real audio (system and microphone as separate WAV files), transcribes it after Stop, and the Recordings, Transcripts and Transcript Detail screens list and open the real session folders. Recordings plays a recording (both tracks mixed, play / pause / seek) but has no speed, volume or output-device choice, and editing transcript text is not built; a run with dev flags but without `--audio` still shows sample data. Progress is tracked in [`ROADMAP.md`](ROADMAP.md) and [`TASKS.md`](TASKS.md), and UI details are in [`UI_PROGRESS.md`](UI_PROGRESS.md).

## Planned capabilities

- Capture system audio (WASAPI loopback) and the microphone as separate tracks.
- Transcribe Thai speech on the device with OpenThai ASR running on ONNX Runtime, with transcript lines shown while recording.
- Keep a local library of recordings and transcripts, indexed in SQLite.
- Export transcripts as TXT, SRT and JSON.

## Screens

| Screen | Purpose |
|---|---|
| Recorder | Start, pause and stop a session; live timer, audio sources with level meters, transcript lines while recording, engine status |
| Recordings | Searchable, sortable library with playback bar, transcribe and delete actions |
| Transcripts | Searchable list of transcripts with title, date, duration, word count, a text preview and export buttons (TXT, SRT, JSON) |
| Transcript detail | Document viewer for a single transcript, with export |
| Settings | Six pages: General, Audio, Transcription, Storage, Appearance, Advanced |
| Modals | Help (shortcut list), About, Delete confirmation |

## Features

- **Frameless window** with a custom title bar, drag, edge and corner resize, minimise, maximise and close, and rounded corners on Windows 11. Minimum size is 1200 x 760.
- **Themes:** dark, light and system, with three accent colours and four font scales. Light theme uses darker text-only colours for legibility.
- **Languages:** the whole interface switches between English and Thai from Settings > General. Text without a Thai translation falls back to English.
- **Fonts:** Inter, JetBrains Mono and Noto Sans Thai are bundled, so Thai renders correctly on any machine.
- **Preferences** (theme, accent, font scale, language and the general toggles) are saved when they change and restored on the next launch.
- **Keyboard:** global shortcuts and a visible focus ring when navigating with Tab.
- **Engine states:** the transcription engine shows loading, ready and unavailable states on the Recorder and in Settings > Transcription. The loading step is simulated for now.

### Keyboard shortcuts

| Shortcut | Action |
|---|---|
| Ctrl + Shift + R | Start or stop recording |
| Ctrl + Shift + P | Pause or resume recording |
| Ctrl + Shift + A | Toggle auto-scroll |
| Ctrl + Shift + C | Copy the transcript |

Shortcuts are ignored while a modal is open. Tab moves focus; Space and Enter activate the focused control.

## Requirements

- Windows 10 or 11. The window rounding and icon handling target Windows; other platforms are untested.
- [Rust](https://www.rust-lang.org/tools/install) stable with the MSVC toolchain (developed with 1.96).
- Windows SDK `rc.exe` on the build machine to embed the `.exe` icon. If it is missing, the build still succeeds with a warning and the executable has no icon.

## Build and run

```powershell
cargo run              # debug build
cargo run --release    # optimised build
```

Quality checks used in this repository:

```powershell
cargo fmt --check
cargo clippy --all-targets
cargo test
```

## Saved settings

Preferences are stored as plain `key=value` text in `%APPDATA%\MeetingRecorder\settings.ini`. The file is written atomically, unknown keys are ignored, and a missing or unreadable file falls back to the defaults. Delete the file to reset all preferences.

## Developer flags

Command-line flags open a specific screen or state, which is how the UI is checked in screenshots. Starting the app with any `--` flag ignores and never writes the saved settings file.

| Flag | Values |
|---|---|
| `--screen=` | `recorder`, `recordings`, `transcripts`, `transcript-detail`, `settings-general`, `settings-audio`, `settings-transcription`, `settings-storage`, `settings-appearance`, `settings-advanced` |
| `--theme=` | `light`, `dark` |
| `--state=` | `idle`, `recording`, `paused`, `processing`, `error` |
| `--modal=` | `help`, `about`, `delete` |
| `--lang=` | `en`, `th` |
| `--engine=` | `loading`, `ready`, `error` |

Example:

```powershell
cargo run -- --screen=recorder --theme=light --lang=th --state=error --engine=error
```

## Project layout

```text
src/
  main.rs               window setup, fonts, icon, entry point
  app.rs                application state, shortcuts, focus ring, per-frame update
  shell.rs              title bar, sidebar, window controls
  theme.rs              colour tokens, fonts, text styles
  i18n.rs               English / Thai dictionary
  branding.rs           app logo and window icon
  settings_store.rs     saved preferences
  modals.rs, toast.rs   Help / About / Delete dialogs, toast messages
  components/           reusable widgets (button, toggle, select, input, table, audio meter)
  screens/              one file per screen; settings/ holds the six settings pages
assets/
  fonts/                bundled fonts and their licences
  icon/                 app logo and .ico
build.rs                embeds the .exe icon
```

## Design

[`design.md`](design.md) is the design specification: colour tokens, typography, spacing, component rules, per-screen layouts and the intentional deviations from the original design. [`DECISIONS.md`](DECISIONS.md) records the technical decisions behind it.

## Development workflow

This repository is developed with an AI coding agent under a documented workflow:

| File | Content |
|---|---|
| [`AGENTS.md`](AGENTS.md), [`CLAUDE.md`](CLAUDE.md), [`CODEX.md`](CODEX.md) | Agent instructions |
| [`WORKFLOW.md`](WORKFLOW.md) | Development lifecycle |
| [`GIT.md`](GIT.md) | Branching and commit rules |
| [`TESTING.md`](TESTING.md) | Testing rules |
| [`SECURITY.md`](SECURITY.md) | Security rules |
| [`PRD.md`](PRD.md) | Product requirements template |
| [`templates/`](templates) | Bug report and feature spec templates |

The user provides requirements and makes the final decisions; the agent handles the rest of the lifecycle and commits or pushes only when authorised.

## Third-party assets

The bundled fonts (Inter, JetBrains Mono, Noto Sans Thai) are licensed under the SIL Open Font License; the licence texts are in [`assets/fonts`](assets/fonts). Icons come from the [Phosphor](https://phosphoricons.com) set through the `egui-phosphor` crate.

## Licence

Copyright 2026 Piya Miang-Lae.

Licensed under the Apache License, Version 2.0. See [`LICENSE`](LICENSE) for the full text. The bundled fonts keep their own SIL Open Font License (see Third-party assets).
