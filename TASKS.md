# Task Tracker

## In Progress

_None._

## Next

- [ ] Task: Audio capture acceptance on a real Zoom or Teams call (open items of the audio capture work)
  - Acceptance Criteria (not yet run, all need a real call or hardware change): in a 2-minute call the other participants are only in `system.wav` and the user only in `mic.wav`; quitting and reopening Zoom/Teams resumes the system track; unplugging the microphone keeps recording and the UI says so; pressing Record during a call captures the meeting app alone (status "Connected"; unverified: whether a call keeps its audio session active while nobody speaks, and if not the track is whole-system); with a meeting app open but silent the status is "All system audio" (done on this machine with an idle Teams); decide whether "Connected" should say that only the meeting app is recorded.
  - Open from the checks that did run: after a crash the WAV headers list only the audio up to the last flush (every 1 s since the preview text work, 5 s before); `recover_header` exists and is tested but nothing calls it (no start-up repair, no UI).
  - Known gap: if a meeting app that is recorded alone quits during a recording, the system track waits for it ("Waiting for the meeting app") and records nothing until a meeting app plays sound again; it does not fall back to the whole system. Only the status on the Recorder says so.
  - Verification: record the call, then inspect both WAVs; record the result per item here.
- [ ] Task: Transcription acceptance items that were not run, or only partly (open items of the transcription work)
  - Not run: a 2-minute Thai speech recorded through the app with a clean capture (the whole-system loopback picks up everything the PC plays, so the clip-through-speakers runs could not prove accuracy); needs a person speaking, or all other audio muted. Expected: CER near the 4.7-5.3% measured on clips.
  - Partly run: resume of an interrupted model download. The app was killed 7 s in; the `.part` file already held the full 883175424 bytes and the next start finished (hash, unzip, `ready.ok`) without a new error. A kill in the middle of the file was not reached (kills at 4 s and 5 s left a 0-byte `.part`), so Range resume from a part-way file is covered by the unit tests only.
  - Partly run: searching `ทำ` finds a line whose text has U+0E4D U+0E32. Covered by unit tests (normalisation and the filter); not driven through the search box of the real window.
  - Known rough edges: the "Recording saved to" toast covers the footer Cancel button for a few seconds; a failed model delete shows "Download failed: ..."; the "Waiting for the model" text and its "Open Transcription settings" button sit slightly apart vertically; the Thai running-state screenshot was not taken; a microphone quieter than -50 dBFS rms is treated as silence; the below-normal priority applies to the worker thread, not to ONNX Runtime's own threads.
  - Verification: do each item and record the result here.
- [ ] Task: Live transcript follow-ups (open items of the live transcript work)
  - Not tried: a real person speaking into the microphone (the 2-minute run played recorded speech through the speakers, so only the system track had speech; the microphone read -90 dB); the model-not-installed path in the real window (unit tests only; the 1 GB model was not deleted to try it); Thai and light-theme screenshots of the "Listening..." state.
  - Known limits: a final line appears only after its segment ends (a pause of 0.5 s after 8 s of speech, 1.2 s after 2 s, or 25 s at most), so with continuous speech the first final line came more than 15 s after the speech began (the grey preview text, see the next entry, fills that gap); a live line can differ from the final line of the same moment (the final transcript cuts and reads the whole file); a recording that starts while a transcription job is on the panel gets no live lines (its final transcript still comes).
  - Verification: do each item and record the result here.
- [ ] Task: Accuracy follow-ups (open items of the language and word-fix work)
  - An ear-checked reference for the 190 s recording `2026-11-25_0157` (the draft is `%USERPROFILE%\Documents\MeetingRecorder\eval\2026-11-25_0157\reference.txt`, the doubtful spots are listed in `check-by-ear.txt` beside it). Every CER below was measured against a reference written from the model's own output and corrected from context only, so it favours the Thai + English greedy output; re-measure once a person has listened.
  - Tried Typhoon Whisper Turbo (`typhoon-ai/typhoon-whisper-turbo`, fp32, PyTorch on CPU, 4 threads, greedy, language Thai; Python only, not ported) on the same 9 segments, punctuation ignored: CER 19.5%, against 9.7% (OpenThai, Thai prompt) and 5.9% (Thai + English prompt). The reference was built from the OpenThai output, so this favours OpenThai; still, Whisper wrote numbers as words ("ห้าพัน", "หนึ่งร้อยเปอร์เซ็นต์"), added "อือ/อ่า" fillers (CER 18.4% without them) and dropped about a fifth of segment 5 (336 against 423 characters). Key terms: "มอก" 2 of 5, "mAh" 1 of 4, "MagSafe" 0 of 1 (OpenThai: 1, 1, 0). Speed: 93 s for 178 s of audio (RTF 0.52, every segment is padded to a 30 s window) against about 0.06 to 0.09. It needs PyTorch and a 1.6 GB model, and its card asks users to accept the OpenTyphoon terms. Not adopted; a fair comparison needs the ear-checked reference first.
  - Word fixes match exactly (Latin letters in any case): every wrong form has to be listed (the run below wrote "มก", which the seeded rules did not list, so "มอก." did not appear). Fixes do not change transcripts made before a rule was added.
  - Verification: do each item and record the result here.
- [ ] Task: Release follow-ups (open items of the update check work)
  - The first `Release` run for tag `v0.0.1` failed on a flaky test (see the CI task). After the fix and the pinned toolchain, run `45654722631` at `bb2d207` passed and made a draft Release; its checksum and zip contents were checked and the exe from the zip started. That build still showed sample data and carried Apache-2.0, so its draft was deleted, `v0.0.1` was moved to `25f593a` (PR #14) and run `45659772587` made the draft that was published on 2026-11-26: <https://github.com/Piya-Boy/Meeting-Recorder/releases/tag/v0.0.1>. Checked after publishing, without logging in: `releases/latest` answers `v0.0.1` (not a draft or pre-release) and the downloaded zip matches `SHA256SUMS.txt`; the released exe read that answer and said it is the latest version.
  - The exe is not signed, so Windows SmartScreen warns on the first run (said in `README.md` and the release notes). Signing needs a code-signing certificate.
  - The collapsed sidebar shows no update hint; the Thai and light-theme screens were not screenshotted.
  - Cut a release: raise `version` in `Cargo.toml` and commit, `git tag vN.N.N`, `git push origin vN.N.N`; the workflow builds a draft Release; check the zip and publish the draft (only a published Release is found by the app). In the same commit, update the README part "Known problems in the `v0.0.1` zip" and the "Changed since" part of `.github/release-notes.md`.
  - Verification: do each item and record the result here.
- [ ] Task: Flaky tests and CI settings (open items of the CI work)
  - `asr::worker::tests::the_model_is_unloaded_after_the_idle_time` failed once in 24 runs of the test binary started 4 at a time (the whole suite under load); 80 parallel runs of that test alone did not fail and the failure message was not kept. It waits 500 ms for a 100 ms idle limit, so a starved worker thread is the likely cause. Catch the message (`cargo test` in a loop with `--nocapture` logging) before changing it.
  - Raise the pinned Rust toolchain (`rust-toolchain.toml`, now 1.96.0) and fix what its clippy adds: on 1.100 `chunks_exact_to_as_chunks` fires in 72 places (`as_chunks` is the suggested form), seen in the first CI run.
  - Set branch protection on `main` to require the `fmt`, `test` and `audit` checks once CI has passed on GitHub (repository settings; not done from here).
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
  - Licence gate (model licence and badge): closed on 2026-11-26 for builds that do not carry the model (`DECISIONS.md`, `THIRD_PARTY_NOTICES.md`).
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

- [x] Task: Show the sentence being spoken as grey preview text (branch `feature/live-partial-text`, on top of `feature/live-transcript`; spec `docs/superpowers/specs/2026-11-25-live-partial-text-design.md`)
  - Built: `SegmentStream::tail`, `LiveTrack::with_partial` / `take_partial_change` (every 2 s of new audio the unfinished tail of a track is read again; a change is reported, a cut withdraws it), `Event::LivePartial { dir, track, line }`, `Command::LiveStart { partial }`, and grey italic rows in the Recorder panel (one per track, in time order, replaced by the final line). The recorder now flushes the WAV header every 1 s instead of 5 s so the live run can read audio that is at most about 1 s old.
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 350 passed, 7 ignored (11 new), stable over 3 runs. Tests were written first and seen to fail (compile errors). Setting the 2 s pace to 0 made `a_partial_is_decoded_again_only_after_two_more_seconds` fail.
  - Acceptance checks run on this machine, real exe, real model, the same recorded Thai clips played through the speakers:
    - PASS the first grey text was on screen when the timer read 00:00:06, about 4 to 5 s after the speech began (the clip starts 1 to 2 s after the Record click); at 00:00:09 it held more words; at 00:00:18 the first segment was a final white line and the next one was grey.
    - PASS at 58 s four final lines were on screen, the grey rows had been replaced by their final lines, and the files held equal frames (5,699,154 each) with a transcript after Stop.
    - CPU: about 0.6 of one core on average over 50 s of recording (CPU time 4.2 s to 35.8 s), about twice the 0.3 measured without preview text. The final job after Stop took about 10 s of wall time.
    - Not run: a person speaking into the microphone; the preview with both tracks speaking at once (two grey rows); Thai and light-theme screenshots.
  - Known limits: the grey text changes while it is shown (words can be rewritten as more audio arrives); each re-read decodes the whole unfinished segment, so CPU grows with segment length (a segment is cut at 25 s at the latest); the grey rows do not follow the search box.
  - Commit: branch `feature/live-partial-text`, not pushed, no PR.

- [x] Task: Thai + English language option and word fixes (branch `feature/language-and-glossary`, on top of `feature/live-partial-text`; spec `docs/superpowers/specs/2026-11-25-language-and-word-fixes-design.md`)
  - Measured first (Python, int8, the 190 s recording `2026-11-25_0157`, 9 segments, reference see the follow-ups task): greedy Thai prompt 9.7% CER, greedy Thai + English prompt 6.0%; beam search (beam 4) 10.7% and 7.8% (it rewrote words both ways: "สรุป" for "สบู่", but "มิถุนายน" for "มิลลิแอมป์"); hotword boosting with beam (β = 1 nat per matched character) 5.55% against 5.97% without it on a CER that counts "mAh" and "มิลลิแอมป์" as the same, and 13% to 19% at β = 2 to 3 (whole ends of segments dropped, a decoy word appeared). Beam search and hotwords were not ported (`DECISIONS.md`).
  - Built: `Language` (Thai prompt 32, Thai + English prompt 63) and `Recognizer::set_language`; `asr::glossary` (one rule a line, `wrong, wrong = right`, longest match first, Latin letters in any case, an empty right side deletes, a result is not replaced again); `TextOptions { language, glossary }` sent with `Command::Transcribe` and `Command::LiveStart` and applied to final lines, live lines and previews (a line the fixes empty is dropped); Settings > Transcription gets a Language select and a "Word fixes" box with a count of fixes and of lines that are not rules; both saved in `settings.ini` (`asr.language`, one `asr.glossary` line per rule).
  - Verification Evidence: `cargo fmt --check`, `cargo clippy --all-targets` clean; `cargo test` 365 passed, 7 ignored (15 new), stable over 3 runs. Tests were written first and seen to fail (compile errors). Applying no fixes in `TextOptions::fix` made three tests fail.
  - Acceptance checks run on this machine, real exe started without flags (so it read `settings.ini`), real model; the user's `settings.ini` was backed up, seeded with `asr.language=th-en` and four rules, and restored afterwards; 80 s of the user's recording `0157` played through the speakers:
    - PASS Settings > Transcription showed "Thai + English" from the file.
    - PASS live text and the final transcript wrote English words in Latin letters ("power bank", "100 percent"); the live lines showed "MagSafe" (the final run wrote "แม็กซ์เซฟ", which no rule listed).
    - PASS both WAVs held 4,158,680 frames; the final transcript was written (4 lines). The test recordings were deleted.
    - Not run: typing rules into the box in the real window (seeded through the file instead), Thai and light-theme screenshots.
    - Found on the way: with `--audio` (a dev flag) the app does not read `settings.ini`; and two clicks of the test driver were lost while sound played until the driver brought the window to the front before each click (automation only).
  - Commit: branch `feature/language-and-glossary`, not pushed, no PR.

- [x] Task: Update check and release workflow (branch `feature/update-check`, from `main`; spec `docs/superpowers/specs/2026-11-25-update-check-design.md`)
  - Built: `update` module (`parse_version`, `is_newer`, `release_url`, `is_safe_release_url`, `release_from_json`, `due`, `check`, `spawn`); the app asks `GET /repos/Piya-Boy/Meeting-Recorder/releases/latest` once per run at start-up when the switch is on and the last good answer is a day old (`update.check` and `update.last_check` in `settings.ini`), says so with a toast and a link above the version in the sidebar, and Settings > Advanced > About has "Check for updates" (always answers) and "Open download page". Settings > General has the "Check for updates" switch (default on). The four hard-coded "0.4.2" texts now come from `Cargo.toml`. `.github/workflows/release.yml` builds and tests on `windows-latest`, zips the exe with `LICENSE`, `THIRD_PARTY_NOTICES.md` and `README.md`, writes `SHA256SUMS.txt`, and on a tag `vN.N.N` (must equal the `Cargo.toml` version) makes a draft Release; started by hand it only uploads the zip as an artifact. Actions are pinned to commit SHAs. The app only notifies: nothing is downloaded or installed.
  - Verification Evidence: `cargo fmt --check` and `cargo clippy --all-targets` clean; `cargo test` 386 passed, 7 ignored (21 new), stable over 12 runs and 20 runs of the test binary 4 at a time (before the last test was added); one earlier run failed one test that I could not identify and did not reproduce (the flaky download test in this file is the likely one). Tests were written first and seen to fail (compile errors). A failed re-check that dropped the version already found was caught in review and fixed test-first. Four mutations each made tests fail: no answer-size limit, every link counted as safe, the start-up check ignoring persistence, and the link taken from the answer instead of built by the app.
  - Acceptance checks run on this machine with the real exe: against a local fake GitHub (a newer version): toast and a sidebar link "Update available: v9.9.9", the Advanced status line and "Open download page" button, request `GET .../releases/latest` with `User-Agent: meeting-recorder/0.4.2`; the same version: "You have the latest version."; real `api.github.com` over TLS (no release exists yet, so 404): "You have the latest version." (a failed automatic check would have shown nothing); with no flags the exe read and wrote `settings.ini`: the first start wrote `update.last_check`, the second start within a day did not check again, and with `update.check=false` it did not check; `settings.ini` was backed up and restored byte for byte. The workflow's PowerShell steps were run locally (version read, tag matches and does not match, zip, SHA-256 of the zip).
  - Found on the way: the About card squeezed the version label when the update buttons shared its row; they moved to a row below.
  - Not run (the workflow ran later, see the release follow-ups): clicking "Open download page" (the test replaces the opener; it runs `explorer <url>`); Thai and light-theme screenshots.
  - Commit: branch `feature/update-check`, not pushed, no PR, no Release.

- [x] Task: CI workflow and Dependabot (branch `ci/add-ci-workflow`, from `main`)
  - Built: `.github/workflows/ci.yml` runs on every pull request to `main` and every push to `main`: `fmt` (`cargo fmt --check`, Linux), `test` (`cargo clippy --all-targets --locked -- -D warnings`, then `cargo test --locked`, on `windows-latest` with `Swatinem/rust-cache`), `audit` (`cargo audit`, Linux). `.github/dependabot.yml` updates the SHA-pinned actions and the cargo dependencies weekly (cargo minor and patch updates in one group). `.cargo/audit.toml` ignores RUSTSEC-2026-0194 and RUSTSEC-2026-0195 (`quick-xml` 0.30, reached only through the Linux accessibility bridge `accesskit_unix`; `cargo tree -i quick-xml@0.30.0 --target x86_64-pc-windows-msvc` is empty) with the reason written beside them. The five other audit findings are "unmaintained" warnings, which do not fail the job.
  - Verification Evidence: both YAML files parse; the exact commands were run locally (`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` clean, `cargo test --locked`); `cargo audit` (installed under the scratchpad) found 2 vulnerabilities before `.cargo/audit.toml` and exits 0 after it.
  - Found on the way: the first Release run (tag `v0.0.1`) failed at `cargo test --locked` on the runner with `asr::download::tests::cancel_keeps_the_part_and_a_second_run_resumes` ("part is 100000 bytes"): the client read the whole file in one read, so the cancel was seen after the file was complete. Not a product bug (a cancel after the last byte just resumes into a finished file); the test assumed the read sizes. Fixed in `test: make the download cancel test independent of read sizes` (branch `fix/download-cancel-test-flake`): the fake server sends 30,000 bytes and holds the connection until the client hangs up; the test also asserts the cancel is prompt (a mutation that ignores the cancel in the read loop made it fail). I could not reproduce the runner's one-read case here (reads came back as 78,840 bytes); the fix does not depend on it.
  - Run on GitHub later: green on PR #13 and on `main` at `bb2d207` (run `45653972346`: test, audit, fmt). Not done: branch protection (required checks `fmt`, `test`, `audit`) is not set.
  - Commit: branch `ci/add-ci-workflow`, not pushed, no PR.

- [x] Task: Release readiness: no sample data, licence, run requirements (branch `fix/release-sample-data`)
  - Found: the exe in the first draft Release showed made-up data to a real user: another name in the Storage and Advanced paths, a "SQLite database" and cache sizes that do not exist, memory and model-cache figures, "184.2 GB free", the sample recording name (every recording made with the app was named after it), a header control that set the recorder's state by hand, and a "Thai (TH)" pill that ignored the Language setting. A first look with dev flags had shown only part of this; the full list came from screenshots of the exe started without flags.
  - Built: Settings > Storage shows the folder recordings really go to; Settings > Advanced keeps only About; "SQLite" is gone from the live screens and About; the recording name starts empty (saved as "recording"); the free-space figure and the state switcher appear only in the preview; the pill follows the Language setting. Existing recordings keep their names.
  - Licence: PolyForm Noncommercial 1.0.0 committed (`DECISIONS.md`). `README.md` has a Download section (Visual C++ Redistributable, SmartScreen, checksum, what does not work yet), and the Release workflow puts `.github/release-notes.md` into the draft.
  - Verification Evidence: render tests draw each main and settings screen and the About modal of an app with a fake sound card and read the painted text; they failed before the change (14 sample values found) and pass after it, and a control test shows the same check does see the sample data in the preview. `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` clean; `cargo test --locked` 394 passed, 7 ignored. The local release exe, started without flags, was screenshotted on every sidebar screen and settings page; `settings.ini` was backed up and restored byte for byte.
  - GitHub: CI green on PR #14 (run `45659195524`) and on `main` at `25f593a` (run `45659690943`); `Release` run `45659772587` passed.
  - Checks on the draft's zip: `sha256sum -c` OK; four files, `LICENSE` is PolyForm Noncommercial; the draft's text starts with the install notes; the exe imports the Visual C++ runtime DLLs and has no delay imports. The CI-built exe, started without flags, was screenshotted on the sidebar screens and settings pages. With `USERPROFILE` pointed at a temporary folder: it transcribed a copy of a 16 s recording (6 s wait, 2 lines), and recorded 5.6 s with an empty name (folder `..._recording`, both tracks 269,593 frames, "No speech found"). The test audio was deleted and `settings.ini` restored.
  - Not run: Thai and light-theme screenshots of the two changed settings pages; a computer without the Visual C++ Redistributable; speech spoken into the microphone with the CI-built exe.

- [x] Task: Open points after `v0.0.1` (branch `fix/open-release-issues`)
  - Controls that did nothing: "Start with Windows", "Minimize to system tray", "Channel configuration", "Runtime", "Test system audio" and "Test microphone" are removed; the "Open-source licenses" button opens the licence texts; the General page no longer says "global shortcuts". Closing the window during a recording asks first.
  - Found on the way, in `v0.0.1`: Settings > Audio had switches of its own that nothing read, so a microphone switched off there was still recorded. Both pages now use one pair of switches, locked while a recording runs.
  - Silent system track: reproduced and fixed. An idle `ms-teams.exe` was picked as the capture target. With one test tone, whole-system capture peaked at 21035 and capture of Teams alone at 0. A meeting app is now captured alone only while it has an active audio session. Linking this to `2026-11-25_1557` and `_1604` is an inference from the start time of Teams (`DECISIONS.md`).
  - Licences: the `v0.0.1` zip has no licence text for the Rust packages, ONNX Runtime or the fonts. `THIRD_PARTY_LICENSES.txt` is generated, shipped in the zip and shown in the app. The model and badge gate is closed for builds that do not carry the model (not legal advice).
  - "Windows 11": the sidebar, the About card, the About dialog and the Appearance page no longer say it. The System theme was always dark; it now follows the Windows setting.
  - Found on the way: a dialog kept the height of the one shown before it (visible after the licence viewer, which is new on this branch). Fixed.
  - Verification Evidence: tests were written first and seen to fail; `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings` clean; `cargo test --locked` 410 passed, 7 ignored.
  - Real exe on this machine (temporary profile; `settings.ini` restored and compared after the runs that used the real one): General and Audio without the removed rows; with the microphone switched off in Settings > Audio a recording made only `system.wav`; the close button during a recording asked, the file kept growing (1,686,554 to 2,002,394 bytes in 3 s), "Keep recording" went on, a Windows close message was held back too, and "Stop and quit" closed the app with a finished WAV header; with an idle Teams running, the fixed build showed "All system audio" and saved a system track with a peak of -1.4 dBFS; in Thai: General, Audio, Transcription, Appearance, Advanced, the licence viewer, the close question, the light theme and the System theme; after the dialog fix, About opened after the licence viewer at its normal height.
  - Not run: the System theme on a light Windows (this Windows is dark and was not changed; covered by unit tests, and `egui-winit` 0.31.1 passes the Windows theme on at start and on change); a real Teams or Zoom call.
  - GitHub: PR #15. At `66a51c3`, run `45671215668` passed `fmt`, `test` (410 passed, 7 ignored), `audit` and `licences` (its first run; the file built on Linux matched the committed one).

- [ ] Task: Thai marks are drawn on top of each other
  - egui 0.31.1 does no text shaping: a tone mark above an upper vowel is drawn over it (in "ทั่วไป" the tone mark covers the vowel). Seen on the real exe in the interface; transcript text is drawn the same way. `v0.0.1` has it. `design.md` asks for tone marks to be tested.
  - Verification: stacked marks are readable in the interface and in a transcript, on screenshots of the real exe.

- [ ] Task: Settings that are not saved
  - These go back to their defaults at every start: the two source switches, the sample rate, "Partial transcription" and "Streaming chunk window". A microphone switched off is on again after a restart.
  - Verification: change each, restart the real exe, and see it kept.

## Rules

- Every task must have a clear outcome.
- Split unrelated features.
- Do not mark tasks complete without verification.
