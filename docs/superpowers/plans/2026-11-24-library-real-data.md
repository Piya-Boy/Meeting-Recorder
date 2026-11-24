# Recordings and Transcripts from Real Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recordings, Transcripts and Transcript Detail show and act on the real session folders instead of sample data.

**Architecture:** A pure `library` module scans `<destination>\<stamp>_<name>\` folders (WAV headers, `transcript.json`). A background scan thread feeds `AppState` over a channel; the three screens render the result in live mode and keep their sample data in preview mode (as the Recorder does with `live`).

**Tech Stack:** Rust 2021, egui 0.31, std threads/mpsc, `serde_json` (new).

**Spec:** `docs/superpowers/specs/2026-11-24-library-real-data-design.md`

## Global Constraints

- No SQLite, no playback, no export, no transcript editing (spec non-goals).
- Never delete a file the app did not write; `delete_session` removes only `mic.wav`, `system.wav`, `transcript.json`, `transcript.txt`, `*.tmp`, then `remove_dir`.
- The UI thread never scans the disk.
- Tests do not assert translated text; new UI strings get a Thai entry in `src/i18n.rs` and the Thai-entry test list.
- `cargo fmt --check`, `cargo clippy --all-targets` (zero warnings), `cargo test` before each commit. Stage by explicit path. No Co-Authored-By lines.
- Line counts, never word counts.

## Review Focus

- A folder with only `mic.wav` (system source was off) lists with that track's duration.
- A zero-byte or truncated WAV lists with 0:00:00 and does not break the scan.
- A damaged `transcript.json` shows Audio only and Transcribe replaces it.
- Delete never touches a folder outside the destination, and keeps unknown files.
- A refresh that finishes late never overwrites a newer one.

---

### Task 1: Library scanning module

**Files:**
- Create: `src/library.rs`
- Modify: `src/main.rs` (`mod library;`), `Cargo.toml` (`serde_json = "1"`)

**Interfaces:**
- Consumes: `crate::asr::wavread::WavReader::{open, rate, total_frames}`.
- Produces:
  - `pub struct TranscriptInfo { pub lines: usize, pub preview: String, pub text: String }`
  - `pub struct Entry { pub dir: PathBuf, pub folder: String, pub name: String, pub stamp: String, pub duration_s: u64, pub bytes: u64, pub transcript: Option<TranscriptInfo> }` (`stamp` is `YYYY-MM-DD HH:MM`)
  - `pub struct Line { pub track: Track, pub start_ms: u64, pub text: String }`, using `asr::transcript::Track`
  - `pub fn scan(base: &Path) -> Vec<Entry>` (newest stamp first; missing base gives empty)
  - `pub fn load_transcript(dir: &Path) -> io::Result<Vec<Line>>`
  - `pub fn delete_session(base: &Path, dir: &Path) -> io::Result<()>`
  - `pub fn format_duration(secs: u64) -> String` (`HH:MM:SS`), `pub fn format_size(bytes: u64) -> String` (`123.4 MB`, `12 KB`)

- [ ] **Step 1: Write failing tests** in `src/library.rs` using temp dirs (`std::env::temp_dir()` + unique name, removed at the end): `scan_lists_a_session_with_both_tracks_and_its_transcript` (two 1 s WAVs written with `audio::wav` or a small helper, a `to_json` transcript with 2 lines; assert name, stamp `2026-11-24 22:48` from folder `2026-11-24_2248_standup`, `duration_s == 1`, `lines == 2`, preview is the first line); `scan_lists_a_mic_only_folder`; `scan_ignores_folders_without_wavs_and_loose_files`; `a_truncated_wav_lists_with_zero_duration`; `a_damaged_transcript_json_is_audio_only`; `scan_sorts_newest_first`; `load_transcript_returns_lines_with_tracks`; `delete_removes_known_files_and_the_folder`; `delete_keeps_unknown_files_and_the_folder`; `delete_refuses_a_path_outside_base` (also `..` and `base` itself); `format_duration_and_size`.
- [ ] **Step 2: Run** `cargo test library` — Expected: FAIL to compile (module missing).
- [ ] **Step 3: Implement** the interface above. Parse `transcript.json` with `serde_json::Value` (version must be 1; line track `"mic"|"system"`; skip lines with a bad shape, fail the file if `lines` is missing). `delete_session` compares `dir.parent() == Some(base)` after canonicalising both.
- [ ] **Step 4: Run** `cargo test library` — Expected: PASS; then `cargo fmt --check && cargo clippy --all-targets && cargo test` all clean.
- [ ] **Step 5: Commit** `git add src/library.rs src/main.rs Cargo.toml Cargo.lock docs/superpowers && git commit -m "feat(library): scan session folders and read transcripts"`.

### Task 2: Background scan and live Recordings screen

**Files:**
- Modify: `src/library.rs` (add `LibraryHandle`), `src/app.rs`, `src/screens/recordings.rs`, `src/modals.rs`, `src/i18n.rs`

**Interfaces:**
- Consumes: Task 1 API; `AppState::start_transcription(dir)`; `RecorderScreen.{asr_job, transcript_dir, state}`.
- Produces:
  - `pub struct LibraryHandle` with `new() -> Self`, `refresh(&mut self, base: PathBuf)` (spawns a scan thread, bumps a sequence number), `poll(&mut self) -> Option<Vec<Entry>>` (returns only the newest finished scan).
  - `RecordingsScreen.live: bool`, `entries: Vec<library::Entry>`, `pub fn visible(&self, now: SystemTime) -> Vec<usize>` applying search, date filter and sort.
  - `AppState.library: LibraryHandle`, `refresh_library()`, and `Modal::DeleteRecording { id, name }` keeps its shape (`id` indexes `entries` by `dir` lookup: store the dir in the modal via a new `dir: PathBuf` field).

- [ ] **Step 1: Write failing tests**: `handle_returns_only_the_newest_scan` (two refreshes of different temp dirs; after both finish, `poll` yields the second's entries); `visible_filters_by_search_date_and_sort` (pure, with entries built by hand); `delete_is_refused_while_recording` and `delete_is_refused_while_the_folder_is_transcribing` (AppState-level helper `delete_recording(&Path) -> Result<(), String>`); `saved_event_and_finished_job_trigger_a_refresh` (observable through a counter on `LibraryHandle`).
- [ ] **Step 2: Run** `cargo test library app::` — Expected: FAIL (items missing).
- [ ] **Step 3: Implement.** In live mode (`go_live` path in `enable_audio`) set `recordings.live = true`, refresh on screen entry, on `Event::Saved`, on `Done/Cancelled/Error` asr events and after delete. Table rows use `Entry` (status from `transcript` / the Recorder job owning `dir`); actions: Open transcript (Task 3 wires the destination), Transcribe → `start_transcription`, trash → modal (or immediate when `confirm_delete` is off), play replaced by an "Open folder" icon button (`explorer.exe <dir>`). Hide the bottom player when `live`. Delete modal and the immediate path both call `AppState::delete_recording`; failure shows a toast. Thai entries for every new string.
- [ ] **Step 4: Run** the new tests, then fmt, clippy, `cargo test`. Then run the exe with `--screen=recordings --audio` and screenshot the list (real folders, Audio only row, delete a throwaway folder through the modal).
- [ ] **Step 5: Commit** `git add src/library.rs src/app.rs src/screens/recordings.rs src/modals.rs src/i18n.rs && git commit -m "feat(recordings): list, transcribe and delete real recordings"`.

### Task 3: Transcripts and Transcript Detail from real data

**Files:**
- Modify: `src/screens/transcripts.rs`, `src/screens/transcript_detail.rs`, `src/shell.rs`, `src/app.rs`, `src/screens/recordings.rs`, `src/i18n.rs`

**Interfaces:**
- Consumes: Task 1 and 2 APIs.
- Produces:
  - `TranscriptsScreen.live`, `entries` (from `AppState`'s scan result, only entries with `transcript`), `visible()` searching title and `text`.
  - `TranscriptDetailScreen.live`, `lines: Vec<library::Line>`, `lines_count`, read-only rendering with the You / Others chip; `transcript_as_text` keeps its signature and uses the new lines (labels via `tr("You")` / `tr("Others")`, pure helper `transcript_text_with(you, others)` for tests).
  - `AppState::open_transcript(&Path)` loads the file (toast on error), fills the detail screen and navigates.

- [ ] **Step 1: Write failing tests**: `transcripts_visible_matches_title_and_line_text` (search `ทำ` matches a line containing it, not only the title); `detail_text_has_one_timestamped_line_per_line_with_speaker` (`transcript_text_with("You","Others")`); `open_transcript_reports_a_damaged_file_and_stays` (AppState with a temp dir holding junk JSON: screen unchanged, notice set).
- [ ] **Step 2: Run** `cargo test transcript` — Expected: FAIL.
- [ ] **Step 3: Implement.** Transcripts rows: title, date, duration, "N lines" (replace the words column header and cell), first-line preview, Open → `open_transcript`, Copy → real text. Detail: replace the editable `TextEdit` with selectable read-only labels, replace the "edit inline ... SQLite" hint with a line count, top-bar title from the entry. Recordings' Open transcript calls `open_transcript`. Preview mode keeps the sample data and the old behaviour.
- [ ] **Step 4: Run** tests, fmt, clippy, full `cargo test`; run the exe, open a real transcript from both lists, screenshot, check Copy all equals `transcript.txt` text.
- [ ] **Step 5: Commit** `git add src/screens/transcripts.rs src/screens/transcript_detail.rs src/shell.rs src/app.rs src/screens/recordings.rs src/i18n.rs && git commit -m "feat(transcripts): show real transcripts and the detail page"`.

### Task 4: Acceptance and documentation

**Files:**
- Modify: `TASKS.md`, `ROADMAP.md`, `DECISIONS.md`, `design.md`, `UI_PROGRESS.md`

**Interfaces:**
- Consumes: Tasks 1-3 built exe.
- Produces: recorded acceptance results and decisions.

- [ ] **Step 1: Acceptance on the real exe**, recording every result including failures: list matches the folders on disk (names, dates, duration within 1 s of Python `wave`, size); a folder created in Explorer appears after reopening the screen; Saved adds a row without reopening; Transcribe on an Audio only row ends as Transcribed; Open transcript shows the same lines as `transcript.txt`; search for a word inside a line finds the transcript; Delete through the modal removes the folder; a delete of a folder with an extra file keeps the file; delete is refused while recording; a 200-folder scan does not stall the window (create copies of one session; note the scan time).
- [ ] **Step 2: Documents.** `TASKS.md` completed entry with evidence plus open tasks (playback, transcript editing, Storage folders feeding the Recorder, index if slow); `DECISIONS.md` entries (scan instead of SQLite, delete scope, `serde_json`); `design.md` section 15 and `UI_PROGRESS.md` updated; `ROADMAP.md` storage line only if everything passed (the SQLite index stays unticked either way).
- [ ] **Step 3: Run** `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` — Expected: clean.
- [ ] **Step 4: Commit** `git add TASKS.md ROADMAP.md DECISIONS.md design.md UI_PROGRESS.md && git commit -m "docs: record library acceptance results and decisions"`.
