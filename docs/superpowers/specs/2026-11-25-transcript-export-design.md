# Export transcripts as TXT, SRT and JSON — design

Date: 2026-11-25. Branch `feature/export-transcripts` (from `feature/asr-transcription`, which holds the library work).
Status: the user answered "ทำเลย" to the proposal "Export TXT / SRT / JSON next", and earlier asked for no more
questions, so this is approved as proposed. Every call below is the author's and is also a ledger ruling.

## Intent

The Export JSON / SRT / TXT buttons on the Transcripts list and on Transcript Detail only show a "not wired"
toast, although the real transcript is on disk with a start and end time per line. A user who wants to put a
meeting into a subtitle editor, a document or a script needs a file, not a toast.

## Goals

- Each of the six buttons (list rows and detail top bar) produces the file for that recording and shows it in
  Windows Explorer with the file selected.
- SRT: numbered cues, `HH:MM:SS,mmm --> HH:MM:SS,mmm`, text prefixed with the speaker label (You / Others).
- TXT: `[hh:mm:ss] You|Others: text`, the same as `transcript.txt`. JSON: the `transcript.json` the app wrote.
- A failure shows a notice and changes nothing else.

## Non-goals

- A "Save as" dialog or choosing the destination (see Approaches).
- Word counts, WebVTT, DOCX, editing the text before export.
- Exports for the sample (preview) rows: they keep the "not wired" toast.

## Anti-goals

- Never overwrite or damage `transcript.json` / `transcript.txt` that the transcription wrote.
- Never leave a half-written export under its final name.
- Delete must still remove everything the app wrote, so `transcript.srt` must be in the delete list.

## Approaches considered

1. Write next to the recording and reveal in Explorer (chosen): no new dependency, no blocking dialog on the UI
   thread, and the file stays with its recording, which is how the library already works. Cost: the user cannot
   pick another folder; they copy the file from Explorer.
2. Native Save dialog (`rfd` crate): the usual desktop feel, but a new dependency and a blocking call that needs
   its own thread plus a channel. Rejected for now; revisit if users ask for a destination.
3. Copy to the clipboard only: already exists as Copy.

## Design

**`src/export.rs`** (pure, no egui). `ExportFormat { Json, Srt, Txt }` with `file_name()`.
`to_srt(lines, you, others) -> String`: cue index from 1; start and end as `HH:MM:SS,mmm` (hours not capped); an
end earlier than the start is raised to the start; newlines in the text become spaces and the text is trimmed
(a blank line would end a cue); a line whose text is empty after trimming is skipped and the numbering stays
continuous. `export_transcript(dir, format, you, others) -> io::Result<PathBuf>`: loads lines through
`library::load_transcript` (so a damaged file is an error); Json returns the existing `transcript.json`; Txt
returns the existing `transcript.txt` and writes it from the lines (atomic) only if it is missing; Srt always
(re)writes `transcript.srt` atomically. `transcript.rs` exposes `write_atomic` and a `lines_to_text(lines, you,
others)` that `to_text` also uses.

**App.** `AppState::export_transcript(dir, format)` calls it with the current UI labels, reveals the file with
`explorer /select,"<path>"`, and sets the notice "Exported. The file is in the recording's folder." or "Could not
export the transcript." The list rows send `TranscriptsRequest::Export(dir, format)`; the detail page keeps the
folder (`dir`) and calls the same method. `library::delete_session` also removes `transcript.srt` and
`transcript.srt.tmp`.

## Constraints

Rust, egui 0.31, std only, Windows. No new dependency. Tests do not assert translated text. New strings get a Thai
entry and a Thai-test key. Never commit to main; stage by explicit path; no Co-Authored-By.

## Left to the builder

Exact notice wording (English plus Thai), how the reveal command is quoted on Windows.
