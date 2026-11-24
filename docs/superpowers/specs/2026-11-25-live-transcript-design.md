# Live transcript while recording — design

Date: 2026-11-25. Branch `feature/live-transcript` (from `main`, which holds PRs #2 to #5).
Status: the user chose "live transcript" from the list of next steps and earlier asked for no more questions, so
this is approved as proposed. Every call below is the author's and is also a ledger ruling.

## Intent

Today the transcript appears only after Stop, and Settings > Transcription shows "Partial transcription" and
"Streaming chunk window" controls that do nothing. A person in a meeting wants to see that the app is hearing
Thai and what it wrote, while the meeting is going on, not minutes later.

## Goals

- While a recording runs, the Recorder's transcript panel fills with lines (You / Others, time stamp) a few
  seconds after each speaker pauses.
- Lines come from the same model and the same segmenter rules as the final transcript.
- The final transcript after Stop is unchanged: it is still made from the saved files, written as
  `transcript.json` and `transcript.txt`, and replaces the live lines on screen. Live lines are a preview and may
  differ a little from it.
- "Partial transcription" off means no live transcript at all (no CPU spent). The "Streaming chunk window" sets
  how often new audio is read for it (500, 1,000 or 2,000 ms).

## Non-goals

- Interim words of a sentence that is still being spoken. The model decodes a whole segment, so a line appears
  when its segment is cut; "partial" here means "lines before the recording ends".
- Writing anything to disk during recording (no live files; a crash leaves only the WAV files, as today).
- Editing live lines, speaker diarization, a live transcript for a recording that is not running.

## Anti-goals

- The recording losing audio because transcription took the CPU (the capture thread is separate and the
  transcription thread is already below-normal priority; this is checked by comparing the frame counts of the
  two files after a recording with live transcription on).
- The window freezing, or a toast or error because the model is not installed (no model: no live lines, and the
  Stop flow shows the same "Waiting for the model" as today).
- A live run that makes the final transcript wrong or late: Stop ends the live run at once, then the normal job
  runs.
- Live lines under the wrong label or out of time order.

## Constraints

Rust, egui 0.31, std threads and channels, Windows. No new dependency. Tests use the fake recognizer and never
need the model; one `#[ignore]` test may use it. New strings get a Thai entry and a Thai-test key. Never commit to
main; stage by explicit path; no Co-Authored-By.

## Approaches considered

1. Run live inside the existing ASR worker thread as a new kind of work (chosen): it already owns the loaded
   model, the queue and the below-normal priority; one thing runs at a time, so a job for an older recording waits
   until the live run ends.
2. A second recognizer on its own thread: doubles the memory (the model takes over 1 GB) and the CPU.
3. Re-transcribe the whole growing file every few seconds: simple but the work grows with the meeting length.
   Rejected; each segment is recognized once.

## Design

- `SegmentStream::poll(on_segment)` (in `asr/segment.rs`): cuts and emits every segment that is already ready in
  the buffered audio using the rules of `find_cut` (a pause after at least 8 s of speech, a longer pause after at
  least 2 s, or the 25 s limit), without flushing the unfinished tail. `push` and `finish` keep their meaning.
- `WavReader::refresh(&mut self) -> io::Result<()>` re-reads the file length so a file that is still being
  written can be followed. The recorder rewrites the header every 5 s and the data lags the same way, so a line
  appears up to about 5 s later than the file was written.
- `asr::live::LiveTrack` follows one file: `LiveTrack::open(path, track)`, `poll(&mut self, rec, out)` reads
  what is new, resamples to 16 kHz, polls the segmenter and recognizes each ready segment into `Line`s.
- Worker: `Command::LiveStart { dir, precision, threads, poll_ms }` and `Command::LiveStop`; events
  `Event::LiveLine { dir, line }` and `Event::LiveEnded { dir }`. Live is queued like a job; while it runs the
  worker keeps accepting commands (they wait, as during a job). `LiveStop` ends it at once. A model that is not
  installed ends it with no error (the app does not start live in that case anyway). The recognizer is loaded
  or reused exactly like a job.
- Audio worker: `Event::SessionStarted { dir }` after the session's files exist, so the app knows the folder.
- App: when `SessionStarted` arrives, the model is `Ready`, the worker exists and "Partial transcription" is on,
  it clears the Recorder panel and sends `LiveStart`. On `Saved` (and on any end of the session) it sends
  `LiveStop` before the existing `Transcribe`. `LiveLine` events add a line to the panel in time order.
- Recorder panel: while recording with live on and no line yet it says "Listening..."; lines look the same as
  final ones. When the final transcript arrives it replaces them.
- Settings > Transcription: the note "…apply to live transcription, which comes later." is replaced by one that
  says what the two controls do.

## Left to the builder

Exact poll loop sleeps, the wording of the new strings (English plus Thai), where the live panel state lives in
`RecorderScreen`.
