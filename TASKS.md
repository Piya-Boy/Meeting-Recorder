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
- [ ] Task: Live transcript during recording (the transcript is made after Stop only)
  - Acceptance Criteria: partial and final lines appear while recording; Settings "Partial transcription" and "Streaming chunk window" take effect (today they are shown but do nothing).
  - Verification: Record 2 minutes with speech and compare the lines with the saved transcript.
- [ ] Task: Library follow-ups (open items of the real-data work)
  - Playback: the Recordings player is hidden for real rows; there is no audio output path yet (the row has an "Open folder" button instead).
  - Editing transcript text on the detail page (read-only for now) and Export TXT / SRT / JSON (buttons still say "not wired").
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
  - Commit: branch `feature/audio-capture`, not pushed, no PR.

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
  - Commit: branch `feature/asr-transcription`, not pushed, no PR.

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

## Rules

- Every task must have a clear outcome.
- Split unrelated features.
- Do not mark tasks complete without verification.
