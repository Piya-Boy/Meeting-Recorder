# Live Transcript Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (native, this session). Steps use checkbox syntax.

**Goal:** The Recorder shows transcript lines while a recording runs, from the same model and segmenter as the final transcript.

**Architecture:** A pollable segmenter and a growing-file reader feed a `LiveTrack` per file; the ASR worker runs them as a new kind of queued work and reports `LiveLine` events; the app starts and stops it from the audio session events and shows the lines in the existing transcript panel.

**Tech Stack:** Rust 2021, egui 0.31, std threads/mpsc. No new dependency.

**Spec:** `docs/superpowers/specs/2026-11-25-live-transcript-design.md`

## Global Constraints

- No new dependency. The UI thread never reads WAV files or runs the model.
- Tests use the fake recognizer and never need the model or an audio device. Tests do not assert translated text; new UI strings get a Thai entry and a Thai-test key.
- Nothing is written to disk by live transcription. The final transcript flow after Stop is unchanged.
- `cargo fmt --check`, `cargo clippy --all-targets` (zero warnings), `cargo test` before each commit. Stage by explicit path. No Co-Authored-By lines.

## Review Focus

- A long silence with no speech never grows memory without bound and never produces an empty line.
- A recording with only one source (only `mic.wav` or only `system.wav`) gives live lines for that source.
- Model not installed or live switched off: no live run, no toast, and Stop behaves exactly as before.
- Stop while the model is recognizing: the live run ends promptly and the final job still writes `transcript.json`.
- Closing the app while live runs exits cleanly; live lines never mix up the You / Others label or the time order.
- The recording's two files have equal frame counts after a recording with live on (capture is not starved).

---

### Task 1: poll the segmenter, follow a growing file, `LiveTrack`

**Files:** modify `src/asr/segment.rs`, `src/asr/wavread.rs`, `src/asr/mod.rs`; create `src/asr/live.rs`.

**Interfaces:**
- Produces: `SegmentStream::poll(&mut self, on_segment: &mut dyn FnMut(usize, &[f32]))`; `WavReader::refresh(&mut self) -> io::Result<()>`; `pub struct LiveTrack`; `LiveTrack::open(path: &Path, track: Track) -> io::Result<LiveTrack>`; `LiveTrack::poll(&mut self, rec: &mut dyn Recognizer, out: &mut Vec<Line>) -> Result<(), String>` (reads the new audio, recognizes each ready segment, appends non-empty `Line`s in 16 kHz-derived milliseconds like the final job).

- [ ] **Step 1: tests first.** `segment`: `poll_emits_ready_segments_and_keeps_the_tail` (speech 3 s, silence 1.5 s, speech 1 s pushed in blocks: after the pause one segment is out; the later speech stays buffered; `finish` then emits it; the union equals `split` of the same audio); `poll_on_silence_only_emits_nothing_and_stays_bounded` (60 s of silence pushed in 1 s blocks: no segment, `buffered_samples` never above 30 s). `wavread`: `refresh_sees_audio_appended_after_open` (write, open, append through a second handle, `refresh`, `total_frames` grows, read returns the new samples). `live`: `a_growing_file_gives_each_segment_once_in_time_order` (fake recognizer; append speech, pause, speech in steps and poll after each: every segment is recognized exactly once, start times increase, the labels are the track's); `a_file_with_no_new_audio_gives_nothing`; `a_recognizer_error_is_returned`.
- [ ] **Step 2: run** `cargo test live segment wavread` → FAIL (does not compile).
- [ ] **Step 3: implement.** `poll` = the cut loop of `finish` without the final flush (stop when `find_cut` is `None` or `0`). `refresh` reads the file length like `open` does and updates `total_frames` (never lowers it). `LiveTrack` owns a `WavReader`, a `Resampler` and a `SegmentStream`; `poll` refreshes, reads every new frame in blocks of at most 30 s, resamples, pushes, polls, and turns each segment into a `Line` with `start_ms = start * 1000 / 16000`.
- [ ] **Step 4: run** `cargo test` → all pass; mutation: make `poll` flush the tail → `poll_emits_ready_segments_and_keeps_the_tail` fails.
- [ ] **Step 5: commit** `git add src/asr/segment.rs src/asr/wavread.rs src/asr/mod.rs src/asr/live.rs && git commit -m "feat(live): follow a growing recording and cut ready segments"`.

### Task 2: the worker runs live transcription

**Files:** modify `src/asr/worker.rs`.

**Interfaces:**
- Consumes: Task 1's `LiveTrack`.
- Produces: `Command::LiveStart { dir: PathBuf, precision: Precision, threads: usize, poll_ms: u64 }`; `Command::LiveStop`; `Event::LiveLine { dir: PathBuf, line: Line }`; `Event::LiveEnded { dir: PathBuf }`.

- [ ] **Step 1: tests first** (existing `Rig` with the fake factory): `live_lines_arrive_while_the_files_grow_and_stop_ends_the_run` (start live on a folder, append speech to `mic.wav` and `system.wav` between polls, `LiveLine`s with the right `track` arrive, `LiveStop` gives `LiveEnded` and no `transcript.json` exists); `a_job_sent_during_live_runs_after_it_ends` (`Transcribe` during live: no `Done` until `LiveStop`, then `Done`); `live_with_a_missing_model_ends_without_an_error`; `live_with_one_track_only_works`; `a_recognizer_error_during_live_is_reported_and_ends_it` (`Event::Error`, then `LiveEnded`); `dropping_the_handle_stops_a_live_run`.
- [ ] **Step 2: run** `cargo test worker` → FAIL.
- [ ] **Step 3: implement.** `accept` queues `LiveStart` like a job and `LiveStop` sets a flag the live loop checks; the loop polls every `poll_ms` (sleeping in small steps so Stop and a closed handle are noticed within 50 ms), loads or reuses the recognizer exactly like `run_job`, sends `LiveLine`s, and ends with `LiveEnded`. Files that do not exist yet are skipped each poll until they appear or the run ends.
- [ ] **Step 4: run** `cargo test`; mutation: do not check the stop flag → the stop test fails.
- [ ] **Step 5: commit** `git add src/asr/worker.rs && git commit -m "feat(live): run live transcription in the ASR worker"`.

### Task 3: session event, app and Recorder wiring, settings text

**Files:** modify `src/audio/session.rs`, `src/audio/worker.rs`, `src/screens/recorder.rs`, `src/app.rs`, `src/screens/settings/transcription.rs`, `src/i18n.rs`.

**Interfaces:**
- Consumes: Task 2's commands and events.
- Produces: `audio::Event::SessionStarted { dir: PathBuf }`; `RecorderScreen::live_started(dir)`, `RecorderScreen::live_line(&Line)`; `RecorderScreen::transcript_empty_state` returns the "Listening..." key while a live run is on.

- [ ] **Step 1: tests first.** audio worker: `starting_a_session_reports_its_folder` (the `SessionStarted` dir contains `mic.wav` or `system.wav`). Recorder: `live_lines_are_inserted_in_time_order_with_their_label`; `a_new_live_run_clears_the_old_lines`; `the_final_transcript_replaces_live_lines`. App (fake worker or event injection, as the existing tests do): `live_starts_when_the_session_starts_the_model_is_ready_and_the_setting_is_on`; `live_does_not_start_with_the_setting_off_or_without_the_model`; `live_stops_before_the_final_job_is_sent` (order of commands seen by the worker); `the_chunk_window_index_maps_to_500_1000_2000_ms`.
- [ ] **Step 2: run** `cargo test` → FAIL.
- [ ] **Step 3: implement.** Emit `SessionStarted` after the session's files exist; the app sends `LiveStart` (poll from the index) and `LiveStop` (on `Saved` and on any session end, before `Transcribe`); `Event::LiveLine` goes to the Recorder panel; replace the settings note with text that says what the two controls do ("Show lines while recording. Lines appear after each pause." / "How often new audio is read for live lines."); new strings get Thai entries and Thai-test keys.
- [ ] **Step 4: run** `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` → all clean; mutation: send `LiveStop` after `Transcribe` → the order test fails.
- [ ] **Step 5: commit** `git add src/audio/session.rs src/audio/worker.rs src/screens/recorder.rs src/app.rs src/screens/settings/transcription.rs src/i18n.rs && git commit -m "feat(live): show transcript lines while recording"`.

### Task 4: real-exe acceptance and docs

- [ ] Real exe with the real model: record about 2 minutes while speech plays, watch lines appear in screenshots during the recording, compare with the final transcript after Stop; check the frame counts of `mic.wav` and `system.wav` are equal (Python `wave`); record with "Partial transcription" off (no lines, final unchanged); stop quickly after start (final still written); model-not-installed behaviour if it can be tried safely; record the CPU use noticed.
- [ ] Update `TASKS.md` (completed entry with real results; remove "Live transcript" from Next), `ROADMAP.md`, `DECISIONS.md` (live runs in the existing worker; preview lines versus the final transcript; what the two settings mean), `design.md` section 15, `UI_PROGRESS.md`, README status (through the index trick: the working copy holds an unrelated licence edit), the spec status line. Commit `docs: record live transcript results and decisions`.
