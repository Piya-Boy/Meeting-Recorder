# Thai transcription after recording (OpenThai ASR on ONNX Runtime) design

Status: implemented on branch `feature/asr-transcription` (PR #3). Open items (clean-capture speech run, mid-file download resume, live transcript, licence gate) are in `TASKS.md` and `DECISIONS.md`.

## Intent and why

The Recorder now writes `system.wav` (the other participants) and `mic.wav` (the user) for each session. The user records Thai meetings and wants the text, kept on this machine. This spec covers turning those two files into a transcript right after the recording stops. Live text while the meeting runs was considered and deferred: the user chose "after Stop first, live later on the same engine", because it is much simpler and the CPU is shared with capture.

## Goals

- After `Saved`, both tracks are transcribed locally with OpenThai 2.0 ASR Streaming (ONNX, int8 by default, fp32 selectable).
- The result is one transcript in time order, each line labelled by track: the mic track is "You", the system track is "Others". No speaker diarization is needed for this.
- The transcript is written next to the audio as `transcript.json` and `transcript.txt`, and shown on the Recorder screen.
- The model is downloaded on first use, with progress, resume, and integrity check. Audio never leaves the machine; the model download is the only network use.
- The UI never blocks: all of this runs off the egui thread, with progress and cancel.
- Thai text is normalized so that search for "ทำ" finds what the model wrote.
- The engine is a separate unit that takes 16 kHz mono samples and returns text, so live transcription can reuse it later.

## Non-goals (this spec)

- Live transcript during recording, partial text, the "Partial transcription" and "Chunk window" settings (they stay UI-only).
- SQLite index, the Recordings and Transcripts screens reading real data, exports (TXT/SRT/JSON export dialogs). The two files written here are what SQLite will index later.
- Speaker diarization inside the system track.
- Echo handling: with speakers instead of headphones the mic picks up the system sound and the same words can appear under both labels. Known limit, not solved here.
- GPU or DirectML inference (the Runtime dropdown stays CPU; DirectML was never checked against this model).
- Correcting English words the model writes in Thai letters, hotwords, punctuation, number formatting ("3" versus "สาม" is left as the model writes it).
- Word counts (Thai has no spaces and no dictionary segmenter is in scope); the UI shows segment count and duration instead.

## Anti-goals (failure even if it technically works)

- A transcript file that looks complete but is not (a cancelled or failed run must leave no `transcript.json`).
- Audio or transcript text sent anywhere, or any download without a pinned hash check.
- The UI freezing, or the recorder dropping audio because transcription took the CPU.
- Dropping words silently when a speaker pauses (the model loses words after long silences if fed a whole file; segmentation exists to prevent this).
- Mis-ordered or mislabelled lines (a "You" line attributed to "Others" or the reverse).

## Constraints

- Windows only, Rust, no async runtime, std threads and `std::sync::mpsc` like the audio worker. All new code under `src/asr/`.
- Model: Hugging Face `iapp/OpenThai2.0-ASR-Streaming-ONNX`, pinned to commit `9f142a43e4d35a422c1c7dac40772ef707387ebc` (seen on 2026-11-24; the builder confirms it still resolves). Licence OpenMDW-1.1.
- Measured on this machine (int8, CPU, 4 threads, 10 FLEURS Thai clips of 5-13 s): CER 4.8%, real-time factor about 0.06, peak RAM 1.26 GiB. fp32: CER 5.2%, RTF 0.08, 2.79 GiB, file about 2.6 GB (int8 about 1.0 GB). Four threads beat eight (the threads contend). These are read-aloud clips in a quiet room, not meeting audio; real meeting accuracy is unmeasured.
- Model interface (from its README, to be re-read by the builder): `encoder.onnx` takes a whole segment as 128-band log-mel `[1,128,T]` at 16 kHz mono (Hann window 25 ms, hop 10 ms, pre-emphasis 0.97) and returns the whole result; text comes from a greedy RNN-T decode. It is not token-streaming through ONNX, which is why audio is cut at silences.
- Input files: mono 16-bit PCM at 48,000, 44,100 or 16,000 Hz (the Settings rate). They can be long (2 hours at 48 kHz is about 690 MB per track), so they are read in blocks, never loaded whole.
- Crates (builder verifies versions and licences): `ort` 2.0.0-rc.x for ONNX Runtime, `realfft` for the FFT, `ureq` (blocking HTTP) and `sha2` for the download. PoodPim used `ort` 2.0.0-rc.15 with statically linked ONNX Runtime; if the `ort` build downloads binaries at build time, that is recorded in `DECISIONS.md`.
- Reused code: PoodPim (MIT, `github.com/thirawat/PoodPim`): `features.rs` (log-mel), `segment.rs` (energy segmenter), `asr.rs` (greedy decode), `model.rs` (download with resume and SHA-256), `audio.rs` (resample). Porting keeps its copyright notice in a `THIRD_PARTY_NOTICES.md` and in the file headers. Its beam search and hotwords are not ported (YAGNI).

## Approaches (decided with the user)

1. **Where it runs.** A second worker thread in the app, driven by commands and events over channels, the same pattern as the audio worker. Rejected: a separate sidecar process (frees RAM afterwards but needs IPC and packaging; 1.26 GiB does not justify it).
2. **Where the model comes from.** Download on first use into `%LOCALAPPDATA%\MeetingRecorder\models\<revision>\<precision>\`. Rejected: bundling in the exe (1 GB+ installer, and the redistribution terms are unchecked).
3. **Code.** Port the needed PoodPim parts into `src/asr/` adapted to our types, tested against the same 10 FLEURS clips used in the spike. Rejected: writing everything from scratch (re-derives tuned values: blank bias, cut rules).

## Design

### Units

- `asr::features`: `log_mel(samples_16k: &[f32]) -> Vec<f32>` laid out `[128, T]`.
- `asr::segment`: streaming energy segmenter. Fed 16 kHz samples in blocks, it yields `Segment { start: usize, end: usize }` sample ranges. Rules ported from PoodPim: the silence threshold is derived from the level of the stretch itself, not a constant; a pause of at least 0.5 s cuts a segment longer than 8 s; a pause of at least 1.2 s cuts a segment longer than 2 s; with no pause at all, a forced cut at 25 s at the quietest point. Segments shorter than a minimum (builder picks, about 0.3 s) are dropped.
- `asr::engine`: `trait Recognizer { fn recognize(&mut self, samples_16k: &[f32]) -> Result<String, AsrError>; }` and `OrtRecognizer` (loads the model with the chosen precision and thread count; greedy decode with blank bias -1.0, and -2.5 for clips under 2 s). The trait lets the worker be tested with a fake.
- `asr::resample`: mono i16 at rate R to f32 at 16 kHz (linear interpolation with a low-pass is enough; builder checks CER does not move versus PoodPim's resampler on the clips).
- `asr::thai`: `normalize(text) -> String`: U+0E4D U+0E32 becomes U+0E33 (the model writes the split form; the combined form is what a user types and searches), runs of whitespace collapse, ends trimmed.
- `asr::model`: model files list with pinned SHA-256 per file (generated by the builder from the pinned revision and committed as constants), `status(dir) -> ModelStatus`, `download(precision, progress, cancel)` with `Range` resume into `*.part`, hash check, atomic rename; a hash mismatch deletes the part and errors.
- `asr::transcript`: types and writers. `Line { track: Track, start_ms: u64, end_ms: u64, text: String }`, `Track::{Mic, System}`. `write(dir, &Transcript)` writes `transcript.json` (below) and `transcript.txt` through temp files and a rename.
- `asr::worker`: `AsrHandle::spawn(config) / send / try_events`, Drop disconnects and joins (like `AudioHandle`).

### Commands and events

```text
Command::DownloadModel { precision }
Command::CancelDownload
Command::Transcribe { dir: PathBuf, precision, threads: usize }
Command::CancelTranscribe
Command::CheckModel { precision }          // answers with ModelStatus

Event::Model(ModelStatus)                  // Missing | Downloading { done, total } | Verifying | Ready | Failed(String)
Event::Engine(EngineEvent)                 // Loading | Ready | Failed(String)  (session load, ~seconds)
Event::Progress { done_secs: f32, total_secs: f32 }   // across both tracks
Event::Done { dir: PathBuf, transcript: Transcript }
Event::Cancelled
Event::Error(String)                       // a failed transcription or download
```

### Flow

1. The app sends `Transcribe { dir }` when the audio worker reports `Saved` and the model is `Ready`; with the model missing it shows a notice and a Download button instead. Several `Saved` in a row queue in FIFO order, one transcription at a time.
2. The worker loads the model once and keeps it for the next job; it unloads after 5 minutes idle to give the RAM back.
3. For each track file that exists (a missing file is a source that was off or failed): read blocks, resample, segment, recognize each segment, normalize, collect `Line`s with times from sample offsets (start of the file is time 0, the same origin as both tracks).
4. Lines of both tracks are merged by `start_ms` (ties: mic first). Then `transcript.json` and `transcript.txt` are written and `Done` is sent.
5. Cancel, an error, or closing the app stops after the current segment and removes any `*.tmp`; no `transcript.json` is left. The WAV files are never touched.

The worker thread runs at below-normal priority and the ONNX thread count comes from Settings > Transcription (default 4; the existing dropdown offers 4, 6 and 8), so capture and the UI keep their share. A new recording may start while a transcription runs.

### Files

`transcript.json` (version 1):

```json
{
  "version": 1,
  "language": "th",
  "model": { "name": "OpenThai2.0-ASR-Streaming-ONNX", "revision": "<commit>", "precision": "int8" },
  "created": "2026-11-24T10:15:00+07:00",
  "tracks": { "mic": "mic.wav", "system": "system.wav" },
  "lines": [ { "track": "mic", "start_ms": 12000, "end_ms": 15800, "text": "สวัสดีครับ" } ]
}
```

`transcript.txt`: one line per `Line`, `[hh:mm:ss] You: text` or `[hh:mm:ss] Others: text` (labels follow the UI language). A transcript with no speech writes an empty `lines` array and a `.txt` with a one-line note, so "ran and found nothing" is distinguishable from "never ran".

### UI changes

- **Recorder, right panel:** with the audio worker connected, the panel shows an empty state ("The transcript appears after you stop recording") until `Done`, then the real lines (You / Others labels, timestamps from `start_ms`). Search, auto-scroll and Copy work on them. With no worker (dev flags) the mock transcript stays. During a job, a progress line replaces the "Latency / CPU" mock footer, with a Cancel button.
- **Recorder, Saved state:** a "Transcribe" button when no job is queued or running for that folder (covers a model installed after the recording). Limit: only the session recorded in this app run, until the Recordings screen reads real folders.
- **Engine badge** ("OpenThai ASR · ONNX Ready"): driven by the real model state. `EngineState` gains `Missing` and `Downloading`.
- **Settings > Transcription:** "Model path" becomes the real model folder (read-only text plus Open folder); a Precision select (int8 default, fp32) and a Download / Delete model control with progress; Threads is real, default 4 threads (the current default index 1, "6 threads (Auto)", changes). Language stays "Thai (TH)", Runtime stays CPU, Partial transcription and Chunk window stay UI-only with a note. Precision and thread choices persist in `settings.ini`.
- **Notices and Thai strings:** every new status and error string gets a Thai entry in `src/i18n.rs`.

### Errors

- Model missing or hash mismatch: `Model(Failed)`, the Recorder shows how to download; recording is unaffected.
- No network or HTTP error: download stops, the `.part` file stays for resume.
- Disk full or folder unwritable when writing the transcript: `Error` with the message; nothing partial is left.
- A WAV that cannot be read (truncated header from a crash): the worker reads by file length as `recover_header` does (it may call that function; it exists and is tested but unused so far), and reports a track it cannot read as an error, keeping the other track's result out of the file unless the user accepts a one-track transcript (builder: decide and record as a ruling; default is to fail the job, never to write half a meeting silently).
- Out of memory while loading the model: `Engine(Failed)`.

## Edge cases the plan must pin with tests

- A track that is entirely silent, or has only noise: empty `lines`, no crash, no invented text.
- A mic file with a speech pause longer than 25 s, and a speech stretch with no pause for more than 25 s: cut rules hold, no word lost at the cuts (compare text with and without cuts on a clip).
- A very short utterance (under 2 s): the stronger blank bias is applied, and an utterance under the minimum is dropped.
- A session with only one track file.
- Both tracks speaking at the same time: lines overlap in time and stay correctly labelled and ordered.
- Resume of a partial download, a corrupted download (hash mismatch), a server that ignores `Range`.
- Cancel during download, during model load, and during a segment: no partial `transcript.json`, worker still usable afterwards.
- Starting a recording while a job runs: the audio files keep equal length (re-check the audio acceptance numbers while transcribing).
- Unicode: the split sara am is normalized; Latin letters and digits in the model output pass through untouched.

## Verification

- Unit tests for features (against reference values the builder computes once with an independent implementation and commits as a small fixture), segmenter rules on synthetic signals, normalization, transcript writer, download logic against a local test server, the worker with a fake `Recognizer`.
- An `#[ignore]` accuracy test over the 10 FLEURS clips (kept outside the repo, path in an environment variable): CER must stay at or below 6% with int8 (spike result 4.8%, so the bar leaves a margin for resampler and segmenter differences).
- Acceptance on this machine, recorded in `TASKS.md` honestly: record 2 minutes of Thai speech (mic) with the app, Stop, download the model, see the transcript on the Recorder and the two files on disk; real-time factor and peak RAM measured during a transcription of at least 10 minutes of real audio; "ทำ" typed in the search box finds a line the model wrote; killing the app mid-transcription leaves no `transcript.json`.

## Release gates (not code)

- Licence: OpenMDW-1.1 (model) against the project's PolyForm Noncommercial: nobody has read the licence text against it. A decision is recorded in `DECISIONS.md` before any release that downloads or redistributes the model.
- OpenThai asks for a "Powered by OpenThai ASR" badge (condition not read yet); the Recorder already shows an "OpenThai ASR" badge, check what is required.
- `THIRD_PARTY_NOTICES.md` with PoodPim's MIT notice and the model's licence text, shown in About.

## Left to the builder

- Exact model file names, sizes and hashes from the pinned revision; the exact tensors of the encoder, decoder and joiner; `ort` version and feature flags.
- Minimum segment length, block sizes, the resampler, the idle-unload timer, and the thread priority call.
- Code layout inside `src/asr/` beyond the units above.

## Calls I made (for the user to check)

- Mic is "You", system is "Others" (a participant joining by phone through the same app is "Others" too).
- The Transcripts and Recordings screens stay mock until the storage spec; this slice shows results on the Recorder only, so an old recording can only be re-transcribed from the session that made it.
- Segment count replaces the word count.
- A failed or cancelled job writes nothing, rather than a partial transcript.
- `ureq` and `sha2` for the download instead of a heavier HTTP stack.
