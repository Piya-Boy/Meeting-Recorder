# Transcript Export Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (native, this session). Steps use checkbox syntax.

**Goal:** The six Export buttons write TXT / SRT / JSON for a real transcript and show it in Explorer.

**Architecture:** Pure `export` module (SRT builder + file producer) over `library::load_transcript`; `AppState::export_transcript` wires it to the list rows (request enum) and the detail page.

**Tech Stack:** Rust 2021, egui 0.31, std.

**Spec:** `docs/superpowers/specs/2026-11-25-transcript-export-design.md`

## Global Constraints

- No new dependency, no Save dialog (spec non-goals). Sample rows keep the "not wired" toast.
- `transcript.json` / `transcript.txt` written by transcription are never overwritten by an export.
- Exports are written atomically (temp file + rename).
- `delete_session` must also remove `transcript.srt` and `transcript.srt.tmp`.
- Tests do not assert translated text; new UI strings get a Thai entry and a Thai-test key.
- `cargo fmt --check`, `cargo clippy --all-targets` (zero warnings), `cargo test` before each commit. Stage by explicit path. No Co-Authored-By lines.

## Review Focus

- A line whose text contains a newline or a blank line does not split an SRT cue.
- A transcript with zero lines exports an empty SRT without error; a damaged `transcript.json` is an error.
- A recording folder whose name has spaces reveals correctly in Explorer.
- Deleting a session after an SRT export leaves no `transcript.srt` behind and removes the folder.
- Exporting TXT twice does not change an existing `transcript.txt`.

---

### Task 1: `export` module and delete scope

**Files:** Create `src/export.rs`; modify `src/main.rs` (`mod export;`), `src/asr/transcript.rs` (pub(crate) `write_atomic`, new `pub fn lines_to_text(lines: &[Line], you: &str, others: &str) -> String` used by `to_text`), `src/library.rs` (delete list).

**Interfaces:**
- Produces: `pub enum ExportFormat { Json, Srt, Txt }` (`Clone, Copy, Debug, PartialEq, Eq`) with `pub fn file_name(self) -> &'static str`; `pub fn to_srt(lines: &[Line], you: &str, others: &str) -> String`; `pub fn export_transcript(dir: &Path, format: ExportFormat, you: &str, others: &str) -> io::Result<PathBuf>`.

- [ ] **Step 1: tests first** in `src/export.rs` `mod tests`:
  - `srt_has_numbered_cues_with_comma_milliseconds_and_speaker`: 2 lines (0/900 ms `a` mic, 3_661_250/3_662_000 ms `b` system) give `1\n00:00:00,000 --> 00:00:00,900\nYou: a\n\n2\n01:01:01,250 --> 01:01:02,000\nOthers: b\n`.
  - `srt_text_never_contains_a_blank_line`: text `"x\n\ny"` and an all-whitespace line give one cue `x y`, numbering continuous.
  - `srt_end_before_start_is_raised_to_start`, `srt_of_no_lines_is_empty`.
  - `export_srt_writes_a_file_and_leaves_json_and_txt_alone`: temp dir written with `asr::transcript::write`; bytes of both unchanged after Srt, Txt and Json exports; returned paths end with the three file names.
  - `export_txt_is_written_when_missing`, `export_of_a_damaged_transcript_is_an_error`.
  - In `library.rs` tests: `delete_removes_an_exported_srt`.
- [ ] **Step 2: run** `cargo test export` → FAIL (module missing).
- [ ] **Step 3: implement** per the spec Design section.
- [ ] **Step 4: run** `cargo test` → all pass; mutation: drop the `transcript.srt` entry from the delete list → `delete_removes_an_exported_srt` fails.
- [ ] **Step 5: commit** `git add src/export.rs src/main.rs src/asr/transcript.rs src/library.rs && git commit -m "feat(export): build SRT and write transcript exports"`.

### Task 2: Buttons, Explorer reveal, Thai strings

**Files:** modify `src/app.rs`, `src/screens/transcripts.rs`, `src/screens/transcript_detail.rs`, `src/shell.rs`, `src/i18n.rs`.

**Interfaces:**
- Consumes: Task 1's `ExportFormat`, `export_transcript`.
- Produces: `TranscriptsRequest::Export(PathBuf, ExportFormat)`; `TranscriptDetailScreen.dir: Option<PathBuf>` (set by `show_real`); `AppState::export_transcript(&mut self, dir: &Path, format: ExportFormat)`.

- [ ] **Step 1: tests first**: in `app.rs` `exporting_a_real_transcript_reports_success_and_a_damaged_one_failure` (notice from `take_asr_notice`; SRT file exists on success); in `transcript_detail.rs` `show_real_remembers_the_folder`.
- [ ] **Step 2: run** → FAIL.
- [ ] **Step 3: implement**: row chips send `Export` when live and the row has a `dir`, otherwise keep the "not wired" toast; the detail bar does the same with `screen.dir`; `export_transcript` reveals the file (`explorer /select,"<path>"` through `raw_arg` on Windows) after a successful export; Thai entries and Thai-test keys for the two notices.
- [ ] **Step 4: run** fmt, clippy, `cargo test` → green; real exe: export all three formats from the list and from the detail page, check the SRT in Explorer and its content, delete the session through the modal and confirm the folder is gone.
- [ ] **Step 5: commit** `git add src/app.rs src/screens/transcripts.rs src/screens/transcript_detail.rs src/shell.rs src/i18n.rs && git commit -m "feat(export): wire the export buttons to real files"`.

### Task 3: Docs

- [ ] Update `TASKS.md` (completed entry with real results; remove Export from the library follow-up), `DECISIONS.md` (write next to the recording, no Save dialog), `design.md` section 15, `UI_PROGRESS.md`, the `README.md` status line only through the index trick (the working copy holds an unrelated licence edit), the spec status line. Commit `docs: record transcript export results and decisions`.
