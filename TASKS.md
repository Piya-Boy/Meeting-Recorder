# Task Tracker

## In Progress

_None._

## Next

- [ ] Task: Audio capture acceptance on a real Zoom or Teams call (open items of the audio capture work)
  - Acceptance Criteria (not yet run, all need a real call or hardware change): in a 2-minute call the other participants are only in `system.wav` and the user only in `mic.wav`; quitting and reopening Zoom/Teams resumes the system track; unplugging the microphone keeps recording and the UI says so; with Zoom and Teams both open and idle the right one is picked (known risk: lowest PID wins).
  - Open from the checks that did run: after a crash the WAV headers list only the audio up to the last 5 s flush; `recover_header` exists and is tested but nothing calls it (no start-up repair, no UI).
  - Verification: record the call, then inspect both WAVs; record the result per item here.
- [ ] Task: Transcription acceptance items that were not run, or only partly (open items of the transcription work)
  - Not run: a 2-minute Thai speech recorded through the app with a clean capture (the whole-system loopback picks up everything the PC plays, so the clip-through-speakers runs could not prove accuracy); needs a person speaking, or all other audio muted. Expected: CER near the 4.7-5.3% measured on clips.
  - Partly run: resume of an interrupted model download. The app was killed 7 s in; the `.part` file already held the full 883175424 bytes and the next start finished (hash, unzip, `ready.ok`) without a new error. A kill in the middle of the file was not reached (kills at 4 s and 5 s left a 0-byte `.part`), so Range resume from a part-way file is covered by the unit tests only.
  - Partly run: searching `ทำ` finds a line whose text has U+0E4D U+0E32. Covered by unit tests (normalisation and the filter); not driven through the search box of the real window.
  - Known rough edges: the "Recording saved to" toast covers the footer Cancel button for a few seconds; a failed model delete shows "Download failed: ..."; the "Waiting for the model" text and its "Open Transcription settings" button sit slightly apart vertically; the Thai running-state screenshot was not taken; a microphone quieter than -50 dBFS rms is treated as silence; the below-normal priority applies to the worker thread, not to ONNX Runtime's own threads.
  - Verification: do each item and record the result here.
- [ ] Task: Live transcript follow-ups (open items of the live transcript work)
  - Not tried: a real person speaking into the microphone (the 2-minute run played recorded speech through the speakers, so only the system track had speech; the microphone read -90 dB); the model-not-installed path in the real window (unit tests only; the 1 GB model was not deleted to try it); Thai and light-theme screenshots of the "Listening..." state.
  - Known limits: a line appears only after its segment ends (a pause of 0.5 s after 8 s of speech, 1.2 s after 2 s, or 25 s at most), so with continuous speech the first line came more than 15 s after the speech began; there are no words for a sentence that is still being spoken; a live line can differ from the final line of the same moment (the final transcript cuts and reads the whole file); a recording that starts while a transcription job is on the panel gets no live lines (its final transcript still comes).
  - Verification: do each item and record the result here.
- [ ] Task: Library follow-ups (open items of the real-data work)
  - Playback follow-ups: listening to one track alone (mic or system), speed and volume (the bar hides the speed chip and the volume icon for real recordings), choosing the output device (it plays through the Windows default), a Thai and a light-theme screenshot of the bar, and a listen-by-ear check of the audio quality, and what happens when you leave the Recordings screen (playback keeps going and the bar, the only controls, is not shown elsewhere; stop on leave or add a mini player). See the playback entry under Completed.
  - Editing transcript text on the detail page (read-only for now).
  - Export: choosing the destination (exports go next to the recording; a Save dialog would need the `rfd` crate and a thread) and WebVTT / DOCX.
  - A folder added or removed outside the app (Explorer) appears only when the screen is opened again: no folder watch and no refresh button.
  - Settings > Storage folders and the "index.db" line are still sample data; the library reads the Recorder's destination.
  - SQLite index: only if listing gets slow (200 sessions scan in 37 ms in a debug build).
  - Verification: do each item and record the result here.
- [ ] Task: Keyboard navigation inside tables and dropdown popups
  - Acceptance Criteria: Arrow keys / Enter work in rows and popups; focus ring visible.
  - Verification: Real key presses with screenshots.

## Blocked

_None._

## Completed

- [x] Task: Implement all 10 Stitch screens and 3 modals as native egui UI
  - Verification Evidence: every screen screenshotted from the running exe and compared with the Stitch render; real-click checks for navigation, delete modal, sidebar collapse, Appearance. See `UI_PROGRESS.md`.
  - Commit: not committed (repository has no commits; user has not asked for one).
- [x] Task: Restyle buttons, dropdowns, playback bar to the Stitch spec; bundle Inter / JetBrains Mono / Noto Sans Thai
  - Verification Evidence: screenshots vs Stitch; fonts and OFL licences in `assets/fonts`.
  - Commit: not committed.
- [x] Task: Frameless window with custom title bar, drag, resize, rounded corners, inset window controls
  - Verification Evidence: scripted real mouse tests (drag, maximise, restore, close, edge/corner resize, 1200x760 minimum).
  - Commit: not committed.
- [x] Task: Light-theme legibility (button text bug, darker semantic text colours, lighter font weights)
  - Verification Evidence: light-theme screenshots of all screens re-reviewed.
  - Commit: not committed.
- [x] Task: English / Thai UI language switching (`src/i18n.rs`)
  - Verification Evidence: all 10 screens and Delete/Help modals shot in Thai; live switch via the dropdown verified by clicking; unit test for lookup.
  - Commit: not committed.
- [x] Task: App logo as window icon, sidebar logo and About logo (`src/branding.rs`)
  - Verification Evidence: zoomed screenshots (dark and light); unit tests for transparent backdrop and icon size. The exe icon was verified separately (see the exe icon task).
  - Commit: not committed.
- [x] Task: Persist user preferences (theme, accent, font scale, language, general toggles)
  - Verification Evidence: unit tests (round trip, bad input); two real launches: language chosen in the dropdown was restored on relaunch; dev-flag runs neither read nor write the file.
  - Commit: not committed.
- [x] Task: App-wide shortcuts (Ctrl+Shift+R / P / A / C)
  - Verification Evidence: real key presses: Ctrl+Shift+R then P produced the Paused state; unit tests for the state transitions.
  - Commit: not committed.
- [x] Task: Transcription engine loading / ready / unavailable states
  - Verification Evidence: screenshots with `--engine=loading|error` on the Recorder and Settings > Transcription; unit tests for the mock load.
  - Commit: not committed.
- [x] Task: Keyboard focus ring
  - Verification Evidence: Tab presses in the running app show the ring on the focused control.
  - Commit: not committed.
- [x] Task: Layouts at other sizes (1200x760, maximised), English and Thai
  - Verification Evidence: screenshots of Recorder, Recordings, Transcripts, Settings pages; fixed overlapping timer and clipped Pause label.
  - Commit: not committed.
- [x] Task: Embed the `.exe` icon (`build.rs`, `assets/icon/app.ico`)
  - Verification Evidence: icon extracted from the built exe shows the logo; version info shows the product name.
  - Commit: not committed.

- [x] Task: Audio capture engine and Recorder wiring (branch `feature/audio-capture`; spec `docs/superpowers/specs/2026-11-24-audio-capture-design.md`)
  - Built: session worker thread behind a `Backend` trait, WASAPI backend (microphone, whole-system loopback, per-process loopback of Zoom/Teams with whole-system fallback), WAV writer with periodic header flush, per-source clocks that pad gaps and rebase after Pause, real device lists, device choice saved by ID, Recorder screen driven by worker events (new Starting and Saved states, source status badges, timer from the worker, real error text).
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 105 passed, 4 ignored. The 4 ignored tests need audio hardware and passed here with `cargo test audio::wasapi_backend -- --ignored --test-threads=1` (microphone capture, whole-system loopback open, process-loopback open; Windows 11 build 26300).
  - Acceptance checks run on this machine, real exe, mouse clicks on Record / Pause / Resume / Stop, no Zoom or Teams running (so the system track was the whole-system fallback):
    - PASS both WAVs are valid mono 16-bit 48 kHz PCM files (parsed with Python `wave`; not listened to). A 4.41 s run gave 211752 frames in each file.
    - PASS track lengths: identical (0 ms) after Stop in the two stopped runs.
    - PASS a 3 s Pause is absent: 3 s record + 3 s pause + 3 s record gave 6.67 s in both files (the extra 0.67 s is the click delays); the timer held at 00:00:03 while paused.
    - PASS starting without Zoom or Teams shows the fallback badge (shown as "Capturing all system audio" in the first run, renamed "All system audio" afterwards and seen again on the Paused screenshot).
    - PARTIAL kill mid-recording: after killing the app at about 7 s both files open, but the headers say 5.01 s (last flush) while the data holds 6.99 s; the track lengths differ by 5 frames. Full length needs `recover_header`, which is not wired (see open task).
    - PASS closing the window during a recording finalizes: after 7 s both files had full headers (336784 frames each, equal) because the worker finalizes when the app drops it.
    - PASS the UI did not stall in any of the 5 recording runs (observed, not measured).
  - Not verified: capturing a real Zoom/Teams call, participant separation between tracks, quit/reopen of the meeting app, microphone unplug, process-loopback timestamps from a live process. These are the open task above; the roadmap item stays unticked.
  - Commit: branch `feature/audio-capture`, PR #2 (open).

- [x] Task: Offline Thai transcription of every saved recording (branch `feature/asr-transcription`; spec `docs/superpowers/specs/2026-11-24-asr-transcription-design.md`, plan `docs/superpowers/plans/2026-11-24-asr-transcription.md`)
  - Built: ONNX Runtime recognizer (OpenThai 2.0 ASR Streaming, ported parts of PoodPim, MIT) behind a worker thread with a queue, cancel and idle unload; model download on first use (pinned revision, SHA-256, resume, zip extract, `ready.ok`); resampling to 16 kHz; pause-based segmentation; Thai text normalisation; merge of the two tracks into `transcript.json` and `transcript.txt` (written atomically, mic = "You", system = "Others"); Settings > Transcription with a real model state, Download / Cancel / Delete, precision and thread count (saved); Recorder shows the transcript, progress, Cancel, "N lines", the "Waiting for the model" notice and failures.
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 246 passed, 6 ignored (the ignored ones need the model, audio hardware or clips).
  - Acceptance checks run on this machine (Intel Core Ultra 5 228V, Windows 11 build 26300), real exe, mouse clicks:
    - PASS accuracy on 10 FLEURS Thai clips, int8, 4 threads, 16 kHz input: CER 5.27% (bar 6%); the same clips converted to 48 kHz and resampled by the app: CER 4.84%, RTF 0.136, peak RAM about 1202 MiB (debug build). The Python spike on the same clips gave 4.8%.
    - PASS 10-minute file (the clips repeated 7 times, 690.5 s) in a release build: CER 4.73%, 33.1 s to transcribe (RTF 0.048), peak RAM 1413 MiB, model load 2.2 s.
    - PASS no model installed: Stop saves both WAVs, no transcript, and the Recorder says "Waiting for the model" with an "Open Transcription settings" button.
    - PASS model download from the Settings button: 946 MiB, about 40 s, files match the manifest sizes (`decoder_joint.onnx` 107330778, `encoder.int8.onnx` 1589571, `encoder.int8.onnx.data` 883175424, `tokens.txt` 93160, `LICENSE` 2619), `ready.ok` written, pill changes to "OpenThai ASR" and the button to "Delete model".
    - PASS killing the app 1.2 s after Stop (transcription running): folder holds only `mic.wav` and `system.wav`; no `transcript.json` or `transcript.txt`.
    - PASS recording a second session while the first is transcribed: both finish; WAV frame counts (Python `wave`) were mic = system in each folder (1544351 and 308442 frames at 48 kHz).
    - PASS transcript on screen equals `transcript.txt` (line counts and text compared in two runs).
    - PARTIAL download resume, search: see the open task above.
    - NOT RUN 2-minute speech with a clean capture: see the open task above.
  - Not verified: speaker labels on a real call (mic = "You", system = "Others" came from the clip runs, where the clips played through the speakers and the whole-system loopback also caught other PC audio).
  - Open gate for any release: licence compatibility of the model (OpenMDW-1.1) with this project's licence, and the "Powered by OpenThai ASR" badge (`DECISIONS.md`, `THIRD_PARTY_NOTICES.md`).
  - Commit: branch `feature/asr-transcription`, PR #3 (open, stacked on #2).

- [x] Task: Recordings, Transcripts and Transcript Detail from real data (branch `feature/library-real-data`; spec `docs/superpowers/specs/2026-11-24-library-real-data-design.md`, plan `docs/superpowers/plans/2026-11-24-library-real-data.md`)
  - Built: `src/library.rs` scans the session folders under the Recorder's destination (WAV durations and sizes, `transcript.json`) on a background thread; Recordings lists them with working search, date filter and sort, Transcribe for audio-only rows, Delete (only the files the app wrote; refused while recording or transcribing) and Open folder; Transcripts lists the transcribed ones (line count, first line, search through every line); the detail page shows the real lines with You / Others chips; Copy and Copy all give the text of `transcript.txt`. Dev-flag runs without `--audio` keep the sample data.
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 277 passed, 6 ignored. Removing a guard made its test fail for: the delete parent check, the sort, the recording-in-progress refusal, the line search.
  - Acceptance checks run on this machine, real exe, mouse clicks:
    - PASS the list matches the disk: durations 43 / 17 / 12 / 20 s and sizes 7.9 / 3.2 / 2.2 / 3.7 MB equal Python `wave` and file sizes for the same folders; dates come from the folder names.
    - PASS a folder created outside the app (copied WAV) was absent while the screen stayed open and listed after leaving and re-opening Recordings.
    - PASS Transcribe on an audio-only row: "Processing 0%", then Transcribed by itself, `transcript.json` and `transcript.txt` written.
    - PASS Delete through the modal removed a session; a folder with an extra user file lost only the WAV and kept the file.
    - PASS Open transcript (from both lists) shows the real lines; Copy all put exactly the text of `transcript.txt` on the clipboard.
    - PASS scan of 200 sessions (100 with transcripts): 37 ms in a debug build (test `scanning_200_sessions_is_quick`); the window was not driven with 200 folders.
    - PARTIAL search inside line text: unit tests only (Thai keyboard input into the real search box was not driven).
    - PARTIAL delete refusal while recording or transcribing: unit tests only, not tried in the real window.
    - PARTIAL "a saved recording adds a row without reopening": unit test only (the list refreshes on `Saved`).
  - Not verified: Thai screenshots of the new rows; keyboard focus on the new row buttons; a transcript of a long meeting on the detail page.
  - Commit: branch `feature/library-real-data` (stacked on `feature/asr-transcription`), not pushed, no PR.

- [x] Task: Export transcripts as TXT, SRT and JSON (branch `feature/export-transcripts`; spec `docs/superpowers/specs/2026-11-25-transcript-export-design.md`, plan `docs/superpowers/plans/2026-11-25-transcript-export.md`)
  - Built: `src/export.rs` builds SRT (numbered cues, `HH:MM:SS,mmm`, `You:` / `Others:` prefix, text flattened to one line so a blank line never splits a cue) and produces the file for a recording. The JSON, SRT and TXT buttons on the Transcripts rows and on the detail page write it next to the recording and show it selected in Explorer. `transcript.json` and `transcript.txt` written by the transcription are never overwritten (an export of those returns them; TXT is written only if missing); `transcript.srt` is rewritten on each export. Delete also removes `transcript.srt` and its temp file. Sample rows keep the "not wired" toast.
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 289 passed, 6 ignored (12 new). Removing `transcript.srt` from the delete list and making TXT always overwrite each made a test fail.
  - Acceptance checks run on this machine, real exe, mouse clicks:
    - PASS SRT from a Transcripts row: file written, content correct (2 cues, `Others:` prefix, comma milliseconds), Explorer opened the folder (name with spaces) with `transcript.srt` selected.
    - PASS SRT, TXT and JSON from the detail page: toast "Exported..." shown; Explorer selected `transcript.txt` and `transcript.json` (read through Shell.Application); the SHA-256 of `transcript.txt` was unchanged.
    - PASS delete through the modal after an export removed the whole folder.
    - PARTIAL failure notice ("Could not export the transcript."): unit test only, not driven in the window.
    - Not verified: Thai screenshot of the toast; opening the SRT in a subtitle player.
  - Commit: branch `feature/export-transcripts` (from `feature/asr-transcription`), not pushed, no PR.

- [x] Task: Play recordings from the Recordings screen (branch `feature/playback`; spec `docs/superpowers/specs/2026-11-25-playback-design.md`, plan `docs/superpowers/plans/2026-11-25-playback.md`)
  - Built: `src/playback.rs`. `Mixer` sums `mic.wav` and `system.wav` (clamped, a shorter track padded with silence, one track alone is fine, a damaged file or different rates is an error). A `Playback` state machine (play, pause, seek, finish) runs over a `Sink` trait on its own thread behind `PlayerHandle`, whose `stop()` and `Drop` wait until the files are closed. The real sink is WASAPI shared mode, event driven, with `autoconvert`, on the Windows default output device. Pause and seek drop what is buffered and rewind the mixer by that much, so nothing is skipped or repeated. Live rows get a play/pause button; the bottom bar shows the title, "Track: System + Mic Mix", elapsed and total time, a seek bar (click, or drag and release) and a close button. `WavReader` gained `seek_frame`.
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 314 passed, 7 ignored (25 new, none needs a sound card; the 7th ignored one is the loopback test below). Tests were written first and seen to fail. Removing the pause rewind, the buffer subtraction in the position, and the player stop before delete each made a test fail. The playback and app tests were stable over repeated runs.
  - Acceptance checks run on this machine, real exe, mouse clicks, on the real recordings (the 43 s one for most steps):
    - PASS play: the bar appeared with the real title and "Track: System + Mic Mix"; the position read 00:00:02, then 00:00:08 about 6 s later, so it follows real time; the row button turned into pause.
    - PASS pause held the position (00:00:03 before and 3 s after); seek by clicking the bar at the middle moved a paused player to 00:00:21 and it stayed paused; resume continued (00:00:23 two seconds later).
    - PASS clicking near the end finished the recording at 00:00:43 of 00:00:43, with the play icon back on the bar and the row.
    - PASS delete while playing (a copy made for the test): the delete modal, then the folder was gone from disk, the bar disappeared, and the three other recordings were untouched.
    - PASS play while recording: the toast "Stop recording before playing."; starting a recording while playing stopped playback with the toast "Playback stopped because a recording started." (the system level meter read -45 dB right after, not the playback).
    - PASS `#[ignore]` loopback test (`cargo test plays_a_tone -- --ignored`): a 440 Hz tone played through `PlayerHandle::start` was heard on the default output device's loopback (440 Hz amplitude 0.235 while playing, 0.013 before; the position advanced 86880 frames in about 1.8 s; no error). The first version measured overall RMS and failed because other sound on this machine already gave 0.084, so it now measures the tone itself.
    - Not verified: how it sounds (no one listened; the loopback proves the tone left the app, not its quality); Thai or light-theme screenshots of the bar; a different output device, or unplugging the device while playing (the failure path is covered by a unit test with a fake sink); a recording whose mic and system files differ in length (unit test only); a long meeting.
  - Known limits: only the mixed track; no speed, volume or output-device choice; one recording at a time; a hot-plugged default device change is not followed.
  - Open (minor): one run of the full suite failed `asr::download::tests::cancel_keeps_the_part_and_a_second_run_resumes` ("part is 100000 bytes": the cancel raced the 100 KB fake file); it passed 8 of 8 alone and 3 of 3 in full runs afterwards. The test is older than this work and depends on timing.
  - Commit: branch `feature/playback` (from `feature/export-transcripts`), not pushed, no PR.

- [x] Task: Live transcript while recording (branch `feature/live-transcript`; spec `docs/superpowers/specs/2026-11-25-live-transcript-design.md`, plan `docs/superpowers/plans/2026-11-25-live-transcript.md`)
  - Built: `SegmentStream::poll` (cuts the ready segments without flushing the tail), `WavReader::refresh`, `src/asr/live.rs` (`LiveTrack` follows one growing WAV: reads what is new, resamples, cuts, recognizes, returns lines), `Command::LiveStart` / `LiveStop` and `Event::LiveLine` / `LiveEnded` in the ASR worker (queued like a job, so one model copy serves both; a job for another recording waits for the live run to end), `Event::SessionStarted { dir }` from the audio worker, and the app and Recorder wiring (the panel shows "Listening..." and then the lines in time order; the final transcript replaces them after Stop). "Partial transcription" off means no live run; "Streaming chunk window" (500, 1,000 or 2,000 ms) is how often new audio is read. Nothing is written to disk during live; the files, `transcript.json` and `transcript.txt` are the same as before.
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 339 passed, 7 ignored. Tests were written first and seen to fail (compile errors). Mutations that each made a test fail: a `LiveStop` that never sets the stop flag (the stop test timed out); not stopping live on `Saved` (the app test failed). The worker and app tests were stable over repeated runs. One full-suite run failed two older worker tests (they expect no model): process ids repeat, and `audio::temp_dir` reused a folder an earlier run had left with a model in it (5,469 folders had piled up in %TEMP%); `temp_dir` now empties its folder first (own commit) and the leftovers were removed.
  - Acceptance checks run on this machine, real exe, mouse clicks, the real model (int8). Speech was 2 minutes of recorded Thai clips played through the speakers and captured by the system track:
    - PASS live lines: at 15 s the panel read "Listening..."; at 59 s it held 3 lines (00:00:04, 00:00:24, 00:00:41); at 2:01 it held 7 lines. After Stop the final transcript replaced them with 8 lines at the same times plus one at 00:01:58; some words differed from the live lines (for example the line at 00:01:24).
    - PASS equal tracks: `mic.wav` and `system.wav` both held 5,837,976 frames (121.62 s) after the live run.
    - PASS Partial transcription off (switched in Settings, then a 23 s recording): the panel kept "The transcript appears after you stop recording" and no line came; after Stop the final transcript was written (1 line).
    - PASS stop about 3 s after start with live on: both WAVs held 117,976 frames and the final transcript was written (1 line).
    - CPU: the process CPU time went from about 2 s to about 38 s over the 2-minute live phase (read from a 5 s sampler; about 0.3 of one core on average), then about 20 s more for the final job. Not measured: the effect on the recording itself beyond the equal frame counts.
    - Not run: see the follow-ups task above.
  - Known limits: see the follow-ups task above.
  - Commit: branch `feature/live-transcript` (from `main`), not pushed, no PR.

## Rules

- Every task must have a clear outcome.
- Split unrelated features.
- Do not mark tasks complete without verification.
