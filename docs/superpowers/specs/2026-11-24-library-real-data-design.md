# Recordings and Transcripts from real data — design

Date: 2026-11-24. Branch `feature/library-real-data` (stacked on `feature/asr-transcription`, PR #3).
Status: the user chose this as the next task, asked for no further questions ("เอาแบบเดิม") and so
approved the proposal made in the conversation (scan the folders, no SQLite for now). Every call
below is the author's; each is also a ledger ruling.

## Intent

After a few weeks of use the user opens Recordings or Transcripts to find an old meeting, read
it, copy it, re-transcribe it or delete it. Today those three screens (Recordings, Transcripts,
Transcript Detail) show hard-coded sample meetings. The app already writes everything needed
to disk, so the screens must show what is really there.

## Goals

- Recordings lists the session folders under the Recorder's destination: name, date, duration,
  size, status (Transcribed / Processing n% / Audio only).
- Open transcript, Transcribe (for Audio only), Delete and "Open folder" work on the real folder.
- Transcripts lists recordings that have a transcript: title, date, duration, number of lines,
  first line as preview. Search looks in the title and in every line.
- Transcript Detail shows the real lines with time stamp and You / Others chip; Copy all copies
  them.
- The lists refresh by themselves after a recording is saved, a transcription finishes and a
  delete.

## Non-goals

- SQLite or any index file: the folders are the data; an index can disagree with the disk the
  moment the user deletes a folder in Explorer. Add one only if listing becomes slow.
- Real playback (needs an audio output path); the mock player is hidden in live mode.
- Export TXT / SRT / JSON (its own roadmap item; the buttons keep their "not wired" toast).
- Editing transcript text (the mock page claimed edits "auto-save to SQLite"; that was false).
- Making Settings > Storage folders feed the Recorder; the library reads the Recorder's
  destination, the single place recordings go today.
- Word counts: Thai has no word boundaries (see `DECISIONS.md`), so the list shows lines.

## Anti-goals

- Never delete a file the app did not write. Never leave a list that shows a recording that is
  gone, or hides one that exists.
- Never freeze the window while listing a large folder.

## Approaches considered

1. Scan the folders in a background thread (chosen): no new state, always true to the disk;
   cost is a header read per WAV on each refresh.
2. SQLite index kept in sync: fast search across everything, but needs sync, migration and a
   repair path for out-of-band deletes. Rejected for now.
3. Scan on the UI thread each frame: simplest, but stalls on big folders. Rejected.

## Design

**`src/library.rs`** (pure, no egui). `scan(base) -> Vec<Entry>` reads each direct subfolder that
holds `mic.wav` or `system.wav`. `Entry { dir, folder, name, stamp, duration_s, bytes,
transcript: Option<TranscriptInfo> }`; `TranscriptInfo { lines, preview, text }` where `text`
is all lines joined for search. Name and stamp come from `<YYYY-MM-DD_HHMM>_<name>` (a folder
that does not match shows its folder name and the folder's modified time). Duration is the
longest track (`WavReader`, so it matches what transcription reads); an unreadable WAV counts as
0 s and the row still shows. A transcript counts only if `transcript.json` parses as version 1
(`serde_json`); a damaged one makes the entry Audio only, and Transcribe replaces it.
`load_transcript(dir) -> Result<Vec<Line>>` for the detail page. `delete_session(base, dir)`
removes only `mic.wav`, `system.wav`, `transcript.json`, `transcript.txt` and `*.tmp` files the
app writes, then `remove_dir`; it refuses a path that is not a direct child of `base`, and a
folder holding other files keeps them (and the folder).

**Scanning.** A `LibraryHandle` runs the scan on a short-lived thread and returns the list over a
channel; `AppState` polls it each frame. A new scan starts when Recordings or Transcripts opens,
on `Event::Saved`, when a transcription ends (Done / Cancelled / Error) and after a delete.
Results from an older scan never replace a newer one (sequence number).

**Status.** Transcribed when the entry has a transcript. Processing when the Recorder's job
owns that folder (Waiting shows 0%, Running shows done/total); an entry with a running job
that has no transcript yet shows Processing. Otherwise Audio only.

**Recordings screen.** In live mode the table is the scan result, filtered by the search box
(name or folder), the date filter (All, Today, Last 7 days, Last 30 days) and the sort
(date newest/oldest, duration, size) — controls that already exist and today do nothing. The
play button and the bottom player are replaced by an "Open folder" icon button. Delete opens the
existing modal (or deletes at once when "confirm before deleting" is off); deleting is refused
with a toast while a recording is in progress or the folder is being transcribed. Empty folder
shows the existing empty state. Preview mode (dev flags) keeps the sample data.

**Transcripts / Detail.** Rows come from entries that have a transcript, newest first. Open loads
`transcript.json` into the detail screen (title, date, duration, "N lines", read-only lines
with the You / Others chip); a load error shows a toast and stays on the list. Copy uses the
same `[hh:mm:ss] You: text` format as `transcript.txt`.

## Constraints

Rust, egui 0.31, std threads and channels, no async. New dependency `serde_json` (parsing
`transcript.json`; the writer stays hand-written). Windows. Tests must not assert translated
text (UI language is process-global). Never commit to main; one feature, atomic commits.

## Left to the builder

Exact column widths and date formatting (reuse the sample's `YYYY-MM-DD HH:MM`), the layout
of the Open-folder button, error wording (English plus a Thai dictionary entry each).
