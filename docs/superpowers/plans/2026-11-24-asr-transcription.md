# Thai transcription after recording Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After a recording is saved, transcribe `mic.wav` and `system.wav` locally with OpenThai 2.0 ASR (ONNX Runtime) and show and save a time-ordered, track-labelled Thai transcript.

**Architecture:** A pure-logic layer (Thai normalization, transcript writers, WAV block reader, resampler, log-mel, segmenter, model manifest and downloader) feeds a `Recognizer` trait (real `OrtRecognizer`, fake in tests). One worker thread (`AsrHandle`, commands and events over `std::sync::mpsc`, like `AudioHandle`) runs download and transcription jobs. `AppState` owns the handle, starts a job when the audio worker reports `Saved`, and the Recorder and Settings > Transcription screens show real state.

**Tech Stack:** Rust 2021, eframe/egui 0.31, std threads, `ort` 2.0.0-rc.x (ONNX Runtime), `realfft`, `ureq` (blocking HTTP), `sha2`, `zip`, `windows-sys` 0.52.

**Spec:** `docs/superpowers/specs/2026-11-24-asr-transcription-design.md`. Reference code (MIT, copy with notice): PoodPim at commit `15e07cc835a6b16b95bea65ae31c3a2eb4c6e856`, `https://github.com/thirawat/PoodPim`, files `src/features.rs`, `src/segment.rs`, `src/asr.rs`, `src/model.rs`, `src/audio.rs` (`resample`). Fetch with `curl -sSL https://raw.githubusercontent.com/thirawat/PoodPim/15e07cc835a6b16b95bea65ae31c3a2eb4c6e856/src/<file>.rs`. GitHub works from this machine; `huggingface.co` file downloads may reset the connection (`ECONNRESET`), so retry and rely on resume.

## Global Constraints

- Windows only, Rust, std threads and `std::sync::mpsc`, no async runtime. New code under `src/asr/`. UI strings in English with a Thai entry in `src/i18n.rs` for every new string; code, comments and docs in English.
- Audio files are never modified. The only network use is downloading the model files below from the pinned `huggingface.co` URL; every file is checked against its pinned SHA-256 before use; no audio or text leaves the machine.
- Model base URL (pinned revision): `https://huggingface.co/iapp/OpenThai2.0-ASR-Streaming-ONNX/resolve/9f142a43e4d35a422c1c7dac40772ef707387ebc`. Files (size in bytes, SHA-256): `encoder.int8.onnx.data` 883175424 `39176685a17087bc45a2a002cdc760951ccc4781693780a59db2b2a05ae964a5`; `openthai2.0-asr-streaming-int8.asr.zip` 109014005 `6714a3333dbbef388aab0a0623a73da19953d9473dc59b98a77dda1a48911553`; `encoder.onnx.data` 2454405120 `a139201333ab89d5e965bc12abb1c534aa285d82e40051c2dda605365c74f18b`; `openthai2.0-asr-streaming-fp32.asr.zip` 149365631 `d4161d14e1c61c779ae01cf02356a99d2752f7931ce2d04be8098c57270d463f`; `LICENSE` 2619 `116493ed1369502b5e29cc559a8fa009a371bae926e769038ebb09885b091221`. Each zip holds the encoder graph (`encoder.int8.onnx` or `encoder.onnx`), `decoder_joint.onnx` and `tokens.txt` (Task 5 confirms the exact entry names and fails loudly if they differ).
- Model folder: `%LOCALAPPDATA%\MeetingRecorder\models\9f142a4\<int8|fp32>\`. Settings keys `asr.precision` (`int8` default, `fp32`) and `asr.threads` (`4` default, `6`, `8`) in `settings.ini`.
- Output files in the session folder: `transcript.json` (version 1) and `transcript.txt`, written through a temp file and a rename. A cancelled or failed job leaves neither file.
- ONNX tensor contract (from PoodPim `asr.rs`, to be re-read): encoder inputs `audio_signal [1,128,T] f32`, `length [1] i64`, `prompt_index [1] i64` (32 = Thai); outputs `encoded`, `encoded_length`. Decoder inputs `encoder_outputs [B,1024,1]`, `targets [B,1] i32`, `target_length [B] i32`, `input_states_1/2 [2,B,640]`; output `outputs`. Greedy RNN-T, max 10 symbols per frame, blank bias -1.0 (-2.5 for audio under 2 s). Log-mel: 16 kHz mono, 128 bands, n_fft 512, window 400, hop 160, pre-emphasis 0.97.
- Segmenter rules (PoodPim `segment.rs`): per-stretch silence threshold; pause of at least 0.5 s cuts a segment longer than 8 s; pause of at least 1.2 s cuts a segment longer than 2 s; forced cut at 25 s at the quietest point.
- Measured targets on this machine (int8, 4 threads): real-time factor about 0.06, peak RAM 1.26 GiB, CER 4.8% on 10 FLEURS clips (accuracy test bar: 6%).
- Git: work on branch `feature/asr-transcription` created from `docs/asr-design`. Never commit to `main`. Conventional Commits, no `Co-Authored-By` lines, no push without the user's go-ahead. The working tree has unrelated uncommitted edits (`AGENTS.md`, `Cargo.toml` license line `Apache-2.0` to `PolyForm-Noncommercial-1.0.0`, `LICENSE`, `README.md`, `skills-lock.json`, untracked `.claude/*`): stage by explicit path, never `git add -A`, never edit `README.md`. For `Cargo.toml`, stage a version with the license line reverted: `sed 's/^license = "PolyForm-Noncommercial-1.0.0"/license = "Apache-2.0"/' Cargo.toml | git hash-object -w --stdin` then `git update-index --cacheinfo 100644,<hash>,Cargo.toml`; check with `git diff --cached Cargo.toml`.
- Before every commit: `cargo fmt --check`, `cargo clippy --all-targets` (zero warnings), `cargo test` (all pass).
- Dev-flag runs (any `--` argument without `--audio`) stay in preview mode: no ASR worker, mock transcript.

## Review Focus

- The model is not downloaded when a recording stops (first run, no network): recording is unaffected, the Recorder says transcription is waiting for the model and offers the download, nothing crashes or hangs. (Task 10)
- A second recording is saved while the first is still being transcribed, or two `Saved` arrive back to back: jobs queue in order, run one at a time, none is lost or run twice; a new recording keeps equal-length tracks. (Task 8)
- The app is closed or killed in the middle of a download or a transcription: the download resumes from its `.part` file and passes the hash check; no `transcript.json` or `.tmp` is left from a job that did not finish. (Tasks 6 and 8)
- A track that is silent, absent, truncated by a crash, or full of noise (user muted, source failed): no invented words, no crash; a track that cannot be read fails the job rather than writing half a meeting. (Tasks 2 and 8)
- Speech with a pause longer than 25 s and speech with no pause for more than 25 s: cut rules hold and no words are lost at the cuts; very short utterances are dropped or decoded with the stronger blank bias. (Task 4)

---

### Task 1: Thai normalization and transcript files

**Files:**
- Create: `src/asr/mod.rs`, `src/asr/thai.rs`, `src/asr/transcript.rs`
- Modify: `src/main.rs` (add `mod asr;` with `#[allow(dead_code)] // wired into the UI in a later task`)

**Interfaces:**
- Produces: `asr::thai::normalize(text: &str) -> String`; `asr::transcript::{Track, Line, Transcript, ModelInfo, write, to_json, to_text, format_clock}`:
  `enum Track { Mic, System }`; `struct Line { track: Track, start_ms: u64, end_ms: u64, text: String }`; `struct ModelInfo { name: String, revision: String, precision: String }`; `struct Transcript { model: ModelInfo, created: String, lines: Vec<Line> }`; `fn merge(mic: Vec<Line>, system: Vec<Line>) -> Vec<Line>` (by `start_ms`, ties mic first); `fn to_json(&Transcript) -> String`; `fn to_text(&Transcript, you: &str, others: &str) -> String`; `fn write(dir: &Path, t: &Transcript, you: &str, others: &str) -> io::Result<()>`.
- Test helper (cfg(test), reuse): `crate::audio::temp_dir(tag)`.

- [ ] **Step 1: Write the failing tests** (in each file's `tests` module)
  - `normalize`: `"ทํา"` (U+0E17 U+0E4D U+0E32) gives `"ทำ"` (U+0E17 U+0E33); `"  สวัสดี   ครับ \n"` gives `"สวัสดี ครับ"`; `"ok 3 ครั้ง"` is unchanged; empty and whitespace-only give `""`.
  - `merge`: mic `[(5000)]` and system `[(1000), (5000)]` give starts `[1000, 5000(mic), 5000(system)]`.
  - `to_json`: output contains `"version": 1`, `"language": "th"`, the model fields, one line object with `"track": "mic"` and the exact Thai text; a text containing `"`, `\` and a newline is escaped so the output parses (check by a tiny hand check of the escaped substrings `\"`, `\\`, `\n`); empty `lines` gives `"lines": []`.
  - `to_text`: `format_clock(3_725_000)` is `"01:02:05"`; a line prints as `[00:00:12] You: สวัสดีครับ`; the system track prints with the `others` label; empty transcript prints one note line (`No speech found.`).
  - `write`: after `write`, `transcript.json` and `transcript.txt` exist and no `*.tmp` is left; writing into a missing directory returns `Err` and leaves nothing.
- [ ] **Step 2: Run** `cargo test asr:: ` Expected: compile errors (items undefined).
- [ ] **Step 3: Implement** the items above. JSON is written by hand (a tested `json_escape`); no serde. `write` writes each file as `<name>.tmp` then renames; `created` is passed in by the caller as an ISO-8601 string.
- [ ] **Step 4: Run** `cargo test asr::` Expected: PASS.
- [ ] **Step 5: Commit** `git add src/asr src/main.rs && git commit -m "feat(asr): add Thai normalization and transcript files"`

### Task 2: WAV block reader and resampler

**Files:**
- Create: `src/asr/wavread.rs`, `src/asr/resample.rs`
- Modify: `src/asr/mod.rs`

**Interfaces:**
- Consumes: `crate::audio::wav::recover_header(path) -> io::Result<u64>` (exists, tested, remove its `#[allow(dead_code)]` here because it is now used), `WavWriter` (tests only).
- Produces: `asr::wavread::WavReader::open(path: &Path) -> io::Result<WavReader>` with `rate(&self) -> u32`, `total_frames(&self) -> u64`, `read_block(&mut self, max_frames: usize) -> io::Result<Vec<f32>>` (empty at end; samples scaled to [-1, 1]). The frame count comes from the file length, not the header, so a file left by a crash is read fully (header sizes are only trusted when they match the length). Rejects anything that is not 16-bit mono PCM with an `InvalidData` error.
  `asr::resample::Resampler::new(from: u32, to: u32) -> Resampler` with `process(&mut self, input: &[f32]) -> Vec<f32>` (stateful across blocks so block boundaries add no clicks) and `fn resample_all(input: &[f32], from: u32, to: u32) -> Vec<f32>`.

- [ ] **Step 1: Write the failing tests**
  - Reader: a file made with `WavWriter` (48,000 frames of a ramp) reads back in 3 blocks of 20,000 with identical values (to 1/32768); `total_frames` 48,000; `rate` 48,000; a stereo or 8-bit file returns `InvalidData`; a file whose header says 0 data bytes but holds 1 s of samples (simulated crash: write samples, do not `finish`) reads 1 s; a file cut in the middle of a sample drops the half sample.
  - Resampler: 48,000 to 16,000 of a 1 kHz sine of 1 s gives 16,000 samples within 1 sample and the dominant frequency stays 1 kHz (zero-crossing count 2,000 within 1%); chunked processing (blocks of 7,001 samples) equals `resample_all` within 1e-4; 16,000 to 16,000 is the identity; 44,100 to 16,000 gives `round(n * 16000 / 44100)` samples within 1; a 12 kHz tone at 48 kHz is attenuated below 0.1 of its amplitude after resampling to 16 kHz (no aliasing into the band).
- [ ] **Step 2: Run** `cargo test asr::wavread asr::resample` Expected: FAIL (undefined).
- [ ] **Step 3: Implement** as specified. Resampler: windowed-sinc or polyphase low-pass plus interpolation; PoodPim `audio.rs::resample` is the starting point but must pass the anti-alias test above.
- [ ] **Step 4: Run** the same tests. Expected: PASS.
- [ ] **Step 5: Commit** `git add src/asr src/audio/wav.rs && git commit -m "feat(asr): add WAV block reader and resampler"`

### Task 3: Log-mel features

**Files:**
- Create: `src/asr/features.rs`, `src/asr/features_reference.rs` (`#[cfg(test)]` constants holding the reference values)
- Modify: `src/asr/mod.rs`, `Cargo.toml` (add `realfft = "3.5.1"`), `THIRD_PARTY_NOTICES.md` (create; PoodPim MIT notice with the copyright line from its `LICENSE` at the pinned commit)

**Interfaces:**
- Produces: `asr::features::{SAMPLE_RATE: u32 = 16_000, N_MELS: usize = 128, log_mel(audio: &[f32]) -> (Vec<f32>, usize)}` returning the features laid out `[128, T]` row-major and `T`, ported from PoodPim `features.rs` with the file-header attribution.

- [ ] **Step 1: Make the reference.** In a scratch cargo project outside the repo (scratchpad), copy PoodPim's unmodified `features.rs`, run it on the fixed signal `x[n] = 0.5*sin(2*pi*440*n/16000) + 0.25*sin(2*pi*3000*n/16000)` for `n in 0..16000` (1 s), and print `T` and the values at `(band, frame)` for bands `[0, 10, 40, 80, 127]` and frames `[0, 1, 50, T-1]`. Commit these 20 numbers and `T` as constants in `src/asr/features_reference.rs` (`#[cfg(test)]`).
- [ ] **Step 2: Write the failing tests**: `shape_matches_reference` (`T` equal, length `128 * T`); `values_match_reference` (each of the 20 values within 1e-3 absolute); `silence_is_constant` (all-zero input gives every value equal and finite); `empty_input_does_not_panic`.
- [ ] **Step 3: Run** `cargo test asr::features` Expected: FAIL (undefined).
- [ ] **Step 4: Port** `features.rs` to `log_mel`; add the attribution header and `THIRD_PARTY_NOTICES.md`.
- [ ] **Step 5: Run** `cargo test asr::features` Expected: PASS.
- [ ] **Step 6: Commit** `git add src/asr Cargo.lock THIRD_PARTY_NOTICES.md` plus the partially staged `Cargo.toml`, `git commit -m "feat(asr): port log-mel features from PoodPim"`

### Task 4: Segmenter and chunked driver

**Files:**
- Create: `src/asr/segment.rs`
- Modify: `src/asr/mod.rs`

**Interfaces:**
- Consumes: `asr::wavread::WavReader`, `asr::resample::Resampler`, `asr::features::SAMPLE_RATE`.
- Produces:
  `asr::segment::split(audio: &[f32], rate: u32) -> Vec<(usize, usize)>` (port of PoodPim `segment::split`, sample ranges);
  `struct SegmentStream` with `SegmentStream::new() -> Self`, `push(&mut self, block_16k: &[f32], on_segment: &mut dyn FnMut(usize, &[f32]))` and `finish(&mut self, on_segment: &mut dyn FnMut(usize, &[f32]))`, which calls back with `(start_sample, samples)` for each completed segment while holding only the unfinished tail in memory (a few seconds, never more than 25 s plus one block). Segments shorter than `MIN_SEGMENT_SAMPLES` (4,800 = 0.3 s) are dropped.
  `fn segment_file(path: &Path, on_segment: &mut dyn FnMut(usize, &[f32]), progress: &mut dyn FnMut(f32, f32) -> bool) -> io::Result<()>` that reads the WAV in 30 s blocks, resamples to 16 kHz, streams segments, reports `(done_secs, total_secs)` after each block, and stops early when `progress` returns `false` (cancel).

- [ ] **Step 1: Write the failing tests** (synthetic signals: speech = 200 Hz sine at amplitude 0.3, silence = amplitude 0.0005 noise)
  - `split` on 3 s speech, 2 s silence, 3 s speech: two segments, boundaries inside the silence.
  - Rule tests: a 10 s speech stretch with one 0.6 s gap in the middle is cut at the gap (stretch above 8 s, pause at least 0.5 s); a 5 s stretch with a 0.6 s gap is not cut (below 8 s); a 5 s stretch with a 1.3 s gap is cut (above 2 s, pause at least 1.2 s); 40 s of continuous speech with no gap is cut into pieces none longer than 25 s.
  - A 30 s gap between two utterances yields two segments and no segment contains the gap.
  - Utterance of 0.2 s is dropped; one of 0.5 s is kept.
  - `SegmentStream` fed the signal in blocks of 1,000, 16,000 and 100,000 samples gives the same segment ranges as `split` on the whole signal (all three block sizes).
  - `segment_file` on a 48 kHz WAV (made with `WavWriter`) calls back with start offsets in 16 kHz samples that match `split` of the resampled signal within 160 samples; `progress` returning `false` after the first block stops the callbacks and returns `Ok`.
  - Peak memory: `SegmentStream` after 10 minutes of alternating speech and silence holds under 30 s of samples (expose `buffered_samples()` for the test).
- [ ] **Step 2: Run** `cargo test asr::segment` Expected: FAIL (undefined).
- [ ] **Step 3: Port** PoodPim `segment.rs` (`Segmenter`, `split`, `quiet_frames`) with attribution, then write `SegmentStream` on top: keep the unfinished tail, and when the segmenter reports a closed segment emit it; at `finish`, emit the remaining tail. Equality with the whole-signal `split` is the contract.
- [ ] **Step 4: Run** `cargo test asr::segment` Expected: PASS.
- [ ] **Step 5: Commit** `git add src/asr && git commit -m "feat(asr): add silence segmenter and streaming file driver"`

### Task 5: Model manifest, status and extraction

**Files:**
- Create: `src/asr/model.rs`
- Modify: `src/asr/mod.rs`, `Cargo.toml` (add `sha2 = "0.11"`, `zip = { version = "9", default-features = false, features = ["deflate-flate2-zlib-rs"] }`; builder checks these versions against PoodPim's `Cargo.toml` and picks ones that resolve)

**Interfaces:**
- Produces:
  `enum Precision { Int8, Fp32 }` with `as_str()` (`"int8"`, `"fp32"`) and `from_str_lossy(&str) -> Precision` (unknown gives `Int8`);
  `struct ManifestFile { name: &'static str, size: u64, sha256: &'static str }`; `fn manifest(p: Precision) -> [ManifestFile; 3]` (data file, zip, LICENSE) using the constants in Global Constraints; `const REVISION: &str`, `const BASE_URL: &str`;
  `fn model_dir(p: Precision) -> PathBuf` (`%LOCALAPPDATA%\MeetingRecorder\models\9f142a4\<precision>`; falls back to the temp dir when the variable is unset) and `fn model_dir_in(base: &Path, p: Precision) -> PathBuf` for tests;
  `enum ModelStatus { Missing, Downloading { done: u64, total: u64 }, Verifying, Ready, Failed(String) }`;
  `fn status(dir: &Path, p: Precision) -> ModelStatus` (`Ready` only when the encoder graph, its `.data`, `decoder_joint.onnx` and `tokens.txt` exist and the sizes match; never rehashes the 2 GB file on every call: a `ready.ok` marker file written after a verified install is the record);
  `fn sha256_file(path: &Path, cancel: &AtomicBool) -> io::Result<Option<String>>` (`None` when cancelled);
  `fn extract_zip(zip: &Path, dir: &Path) -> Result<(), String>` (each entry through `<name>.part` then rename, only the file name part of an entry is used, entries with `..` are skipped; fails if `decoder_joint.onnx`, `tokens.txt` or the encoder graph is missing afterwards).

- [ ] **Step 1: Write the failing tests**
  - `manifest` has the exact sizes and hashes from Global Constraints for both precisions (assert each string); `Precision::from_str_lossy("fp32")` is `Fp32`, `""` and `"x"` are `Int8`.
  - `model_dir_in(base, Int8)` ends with `9f142a4\int8`.
  - `sha256_file` of a file holding `abc` is `ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad`; a set `cancel` flag returns `Ok(None)`.
  - `extract_zip` on a small zip built in the test (entries `encoder.int8.onnx`, `decoder_joint.onnx`, `tokens.txt`, and a hostile `../evil.txt`) writes the three files, never writes `evil.txt` outside `dir`, leaves no `.part`; a zip missing `tokens.txt` returns `Err` naming it.
  - `status`: empty dir gives `Missing`; all files present and `ready.ok` gives `Ready`; a data file of the wrong size gives `Missing` (not `Ready`); no `ready.ok` gives `Missing`.
- [ ] **Step 2: Run** `cargo test asr::model` Expected: FAIL (undefined).
- [ ] **Step 3: Implement.** Confirm the real zip entry names by downloading the int8 zip once by hand (`curl -L -o`; retry on reset) and listing it with the `zip` crate in a scratch test; if the names differ from `encoder.int8.onnx`, `decoder_joint.onnx`, `tokens.txt`, change the constants and add the observed names to the ledger.
- [ ] **Step 4: Run** `cargo test asr::model` Expected: PASS.
- [ ] **Step 5: Commit** `git add src/asr Cargo.lock` plus partially staged `Cargo.toml`, `git commit -m "feat(asr): add model manifest, status and zip extraction"`

### Task 6: Model download with resume

**Files:**
- Modify: `src/asr/model.rs`, `Cargo.toml` (add `ureq = { version = "3", default-features = false, features = ["native-tls"] }`; PoodPim uses 3.5.2 with `native-tls` so no C toolchain is needed)

**Interfaces:**
- Consumes: Task 5 items.
- Produces: `fn download(dir: &Path, p: Precision, base_url: &str, progress: &dyn Fn(u64, u64), cancel: &AtomicBool) -> Result<(), String>` (`base_url` is a parameter so tests can point at a local server; the app passes `model::BASE_URL`). For each manifest file: skip when present with the right size and hash marker; otherwise fetch through `<name>.part` with `Range: bytes=<len>-` resume, up to 6 attempts, a 45 s stall timeout, then verify the SHA-256 (mismatch deletes the part and returns `Err`), rename into place; then extract the zip, delete it, write `ready.ok`. `progress(done, total)` counts bytes over all three files. Returns `Err("cancelled")` when `cancel` is set (the `.part` stays for resume).

- [ ] **Step 1: Write the failing tests** against a tiny HTTP server built in the test with `std::net::TcpListener` serving a fake model (three small files with their real SHA-256 computed in the test, a small zip) and the manifest passed through a test-only `download_files(dir, files: &[ManifestFile-like], ...)` seam so real sizes are not needed (the public `download` calls the same function with the real manifest)
  - Full download writes all files, extracts, writes `ready.ok`; `status` then says `Ready`.
  - Resume: a `.part` holding the first 40% of a file is continued (the server sees `Range: bytes=<n>-` and sends `206`); final hash is correct.
  - A server that ignores `Range` and sends `200` with the whole body: the code restarts the file from zero and still ends with a correct file.
  - Corrupted body (one flipped byte): `Err` mentioning the file, the `.part` and the final file are removed.
  - A connection dropped halfway is retried and completes within the 6 attempts; a server that always drops fails with `Err` after 6 attempts (use a short attempt delay in tests).
  - Cancel: setting the flag from a progress callback stops with `Err("cancelled")` and keeps the `.part`; a following `download` with the flag cleared resumes and succeeds.
  - Progress is monotonic and ends at `total`.
- [ ] **Step 2: Run** `cargo test asr::model::download` Expected: FAIL.
- [ ] **Step 3: Implement** with `ureq` using the OS TLS stack; reference PoodPim `model.rs::fetch` for the resume and stall logic.
- [ ] **Step 4: Run** `cargo test asr::model` Expected: PASS.
- [ ] **Step 5: Commit** `git add src/asr Cargo.lock` plus partially staged `Cargo.toml`, `git commit -m "feat(asr): add resumable model download"`

### Task 7: Recognizer trait and ONNX engine

**Files:**
- Create: `src/asr/engine.rs`, `tests/asr_accuracy.rs` (integration test, `#[ignore]`)
- Modify: `src/asr/mod.rs`, `Cargo.toml` (add `ort = { version = "2.0.0-rc.15", default-features = false, features = ["std", "tls-native", "api-31", "download-binaries"] }`; if that exact set does not build on Windows MSVC, use what PoodPim's `desktop` feature uses and record the change; note in the ledger that `download-binaries` fetches ONNX Runtime at build time)

**Interfaces:**
- Consumes: `asr::features::log_mel`, `asr::model::{model_dir, Precision, status}`, `asr::thai::normalize`.
- Produces: `trait Recognizer: Send { fn recognize(&mut self, samples_16k: &[f32]) -> Result<String, String>; }`; `struct OrtRecognizer` with `OrtRecognizer::load(dir: &Path, threads: usize) -> Result<OrtRecognizer, String>` (port of the greedy path of PoodPim `asr.rs::OpenThaiAsr::{load_with, transcribe}`: prompt index 32, blank bias -1.0 and -2.5 under 2 s, token text from `tokens.txt`, PoodPim's pause marker handling; beam search and hotwords are not ported); `recognize` returns normalized text (`thai::normalize` applied) and `Ok("")` for audio the model finds no speech in.
  Test-only: `struct FakeRecognizer` (`Vec<String>` of canned answers, records the lengths of the audio it received).

- [ ] **Step 1: Write the failing tests**
  - Unit (no model): `FakeRecognizer` returns canned answers in order and records input lengths; `recognize` on an empty slice returns `Ok("")` without calling the model (guard in `OrtRecognizer`, tested through a `decode_tokens(ids, tokens)` pure function: ids to text with `▁` and pause marker handling, using a 6-entry fake token table); the blank-bias selector returns -2.5 for 1.9 s and -1.0 for 2.0 s.
  - `#[ignore]` `loads_and_transcribes_a_clip`: needs `ASR_MODEL_DIR` (a ready int8 model folder); transcribes a 5-13 s clip from `ASR_TEST_CLIPS` and returns non-empty Thai text.
  - `#[ignore]` `fleurs_cer_stays_below_6_percent` in `tests/asr_accuracy.rs`: reads `ASR_MODEL_DIR`, `ASR_TEST_CLIPS` (folder of `<name>.wav` 16 kHz mono and `<name>.txt` reference), runs the full chain `segment_file`, `recognize`, joins, computes character error rate after removing spaces, asserts at most 0.06 and prints the real-time factor and the peak RAM.
- [ ] **Step 2: Run** `cargo test asr::engine` Expected: FAIL (undefined).
- [ ] **Step 3: Implement.** Get the model for the ignored tests once: write a small `#[ignore]` test `download_model_for_tests` that calls `model::download` into `%LOCALAPPDATA%\MeetingRecorder\models\...` and prints the folder (run with `cargo test asr::model::download_model_for_tests -- --ignored --nocapture`); a hand-run is fine, it is 1 GB. FLEURS clips come from the public dataset `google/fleurs` (`th_th`, CC-BY 4.0) and stay outside the repo.
- [ ] **Step 4: Run** `cargo test asr::engine` (pure tests pass), then `ASR_MODEL_DIR=... ASR_TEST_CLIPS=... cargo test --test asr_accuracy -- --ignored --nocapture`. Expected: PASS with CER at most 0.06; write the CER, RTF and RAM into the ledger. If CER is above 0.06, stop and debug with `superpowers:systematic-debugging` (features or tokens first); do not loosen the bar.
- [ ] **Step 5: Commit** `git add src/asr tests/asr_accuracy.rs Cargo.lock` plus partially staged `Cargo.toml`, `git commit -m "feat(asr): add ONNX recognizer ported from PoodPim"`

### Task 8: ASR worker

**Files:**
- Create: `src/asr/worker.rs`
- Modify: `src/asr/mod.rs`

**Interfaces:**
- Consumes: `Recognizer`, `segment_file`, `transcript::{Line, Track, Transcript, ModelInfo, merge, write}`, `model::{Precision, ModelStatus, status, download, model_dir}`.
- Produces:
  `enum Command { CheckModel { precision: Precision }, DownloadModel { precision: Precision }, CancelDownload, Transcribe { dir: PathBuf, precision: Precision, threads: usize }, CancelTranscribe }`;
  `enum Event { Model(ModelStatus), Engine(EngineEvent), Progress { done_secs: f32, total_secs: f32, dir: PathBuf }, Done { dir: PathBuf, transcript: Transcript }, Cancelled { dir: PathBuf }, Queued { dir: PathBuf, position: usize }, Error { dir: Option<PathBuf>, message: String } }` with `enum EngineEvent { Loading, Ready, Failed(String) }`;
  `struct AsrConfig { idle_unload: Duration, base_url: String, models_base: Option<PathBuf>, labels: (String, String) }` with `Default` (5 min, `model::BASE_URL`, `None`, English labels);
  `trait RecognizerFactory: Send + Sync { fn load(&self, dir: &Path, threads: usize) -> Result<Box<dyn Recognizer>, String>; }`, real `OrtFactory`, test `FakeFactory`;
  `AsrHandle::spawn(factory: Arc<dyn RecognizerFactory>, config: AsrConfig) -> AsrHandle`, `send(&self, Command)`, `try_events(&self) -> Vec<Event>`; `Drop` disconnects, cancels the running job and joins. The worker thread sets itself to below-normal priority (`SetThreadPriority(THREAD_PRIORITY_BELOW_NORMAL)` under `cfg(windows)`).

- [ ] **Step 1: Write the failing tests** (Rig harness waiting on events like the audio worker tests; `FakeFactory` returns a `FakeRecognizer` that answers `"mic-<n>"` or `"sys-<n>"` by call order per track via a wrapper that knows the track from the audio length, or a canned list; WAVs made with `WavWriter` of speech-like segments; use a fake model dir with `ready.ok` and the three files)
  - Both tracks: `Transcribe` on a folder with `mic.wav` and `system.wav` gives `Done` with lines from both tracks merged by start time (mic first on ties) and times within 50 ms of the sample offsets; `transcript.json` and `transcript.txt` exist after `Done`; labels follow `AsrConfig.labels`.
  - Only `mic.wav` present: `Done` with mic lines only; neither file present: `Error` and no transcript files.
  - Silent files: `Done` with an empty `lines` array, files written, `FakeRecognizer` never called.
  - A truncated `system.wav` (header not finalized): read by length, still transcribed; a file that is not a valid 16-bit mono WAV: `Error` naming the file, no transcript files written (the job fails rather than writing one track silently).
  - Progress: events with `done_secs` rising to `total_secs`, one `Engine(Loading)` then `Engine(Ready)` for the first job and none for the second job (model stays loaded).
  - Queue: two `Transcribe` sent back to back give `Queued { position: 1 }` for the second; two `Done` in order; both folders have transcripts; the same folder sent twice while queued runs once.
  - Cancel: `CancelTranscribe` during the first segment gives `Cancelled`, no `transcript.json`, no `*.tmp`; the next `Transcribe` still works; dropping the handle during a job joins within 5 s and leaves no `transcript.json`.
  - Recognizer error on one segment: the job fails with `Error` and no transcript files (no half transcript).
  - Model missing: `Transcribe` when `status` is not `Ready` gives `Error` with the message `Model is not installed` and no `Engine` events.
  - `CheckModel` answers with `Event::Model(status)`; `DownloadModel` against a local test server (Task 6 helper) gives `Model(Downloading..)` events then `Model(Ready)`; `CancelDownload` gives `Model(Failed("cancelled"))` and keeps the `.part`.
  - Idle unload: with `idle_unload` 100 ms the model is dropped after the idle time and the next job emits `Engine(Loading)` again.
- [ ] **Step 2: Run** `cargo test asr::worker` Expected: FAIL (undefined).
- [ ] **Step 3: Implement** one worker thread with a job queue, a cancel flag per running job, the loaded recognizer cached with a last-used instant, and download jobs run on the same thread (a download blocks transcription, which is acceptable: a transcription needs the model anyway). Transcript `created` comes from `audio::naming::local_stamp`-style local time (add a small ISO formatter if needed; non-Windows stub like `local_stamp`).
- [ ] **Step 4: Run** `cargo test asr::worker` Expected: PASS.
- [ ] **Step 5: Commit** `git add src/asr && git commit -m "feat(asr): add transcription worker with queue, cancel and idle unload"`

### Task 9: Settings and engine state

**Files:**
- Modify: `src/settings_store.rs`, `src/screens/settings/transcription.rs`, `src/screens/recorder.rs` (`EngineState`), `src/app.rs`, `src/i18n.rs`

**Interfaces:**
- Consumes: `asr::model::{Precision, ModelStatus, model_dir}`, `asr::worker::{AsrHandle, Command, Event}`.
- Produces: `Persisted { asr_precision: String, asr_threads: u32 }` (keys `asr.precision`, `asr.threads`; defaults `"int8"`, `4`; unknown values fall back to defaults); `EngineState` gains `Missing` and `Downloading` (existing `Loading`, `Ready`, `Error` keep their meaning); `SettingsTranscriptionScreen` gains `precision: Precision`, `model_status: ModelStatus`, and `fn threads(&self) -> usize` (index 0, 1, 2 map to 4, 6, 8; default index 0); `AppState::enable_asr(handle: AsrHandle)` stores it, sends `CheckModel`; `AppState::apply_asr_event(Event)`; the worker's `Model` events set `settings_transcription.model_status` and `recorder.engine` (`Ready` to `Ready`, `Missing` to `Missing`, `Downloading` to `Downloading`, `Verifying` and `Engine(Loading)` to `Loading`, `Failed` and `Engine(Failed)` to `Error`).

- [ ] **Step 1: Write the failing tests**
  - Store: round trip with new keys; missing keys default to `int8` and `4`; `asr.threads=7` falls back to `4`; `asr.precision=weird` falls back to `int8`.
  - Screen: `threads()` is 4, 6, 8 for indexes 0, 1, 2; default index gives 4; changing precision in the screen makes `AppState` send `CheckModel` for the new precision (observed through a test-side channel, using `AsrHandle` over `FakeFactory`).
  - App: `apply_asr_event(Model(Ready))` makes `recorder.engine == Ready` and `model_status == Ready`; `Model(Downloading{done: 50, total: 100})` gives `Downloading`; `Engine(Failed("x"))` gives `Error`; persistence snapshot includes precision and threads.
  - i18n: every new string has a Thai entry (`has_thai`): `Missing`, `Downloading`, `Download model`, `Delete model`, `Model folder`, `Precision`, `Open folder`, `Model is not installed`, `Model downloaded`, `Download failed`, `Not downloaded`, plus the engine pill texts you add.
- [ ] **Step 2: Run** `cargo test` Expected: FAIL (compile errors).
- [ ] **Step 3: Implement.** Settings > Transcription: "Model path" becomes read-only text of `model_dir(precision)` with an Open folder button (`explorer.exe` with the folder, only when it exists); a Precision select (`int8 (default, about 1 GB)`, `fp32 (about 2.6 GB)`); a Download model button (sends `DownloadModel`, becomes Cancel while downloading, with a progress bar and MB text) and a Delete model button (removes the folder with a confirmation toast text; only enabled when `Ready` and no job running); the Runtime select stays but only `CPU` is selectable; Partial transcription and Chunk window stay UI-only with a one-line note that they apply to live transcription later. The ASR pill reflects `EngineState`.
- [ ] **Step 4: Run** `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` Expected: all pass.
- [ ] **Step 5: Screenshots** of Settings > Transcription in each state (`--screen=settings-transcription` with an `--engine=missing|downloading|ready` flag you add to `AppState::from_args`), dark and light, English and Thai; check long folder paths do not break the layout.
- [ ] **Step 6: Commit** `git add src/settings_store.rs src/screens src/app.rs src/i18n.rs && git commit -m "feat(asr): add transcription settings and real engine state"`

### Task 10: Recorder wiring

**Files:**
- Modify: `src/screens/recorder.rs`, `src/app.rs`, `src/main.rs`, `src/i18n.rs`; remove the `#[allow(dead_code)]` on `mod asr;` in `src/main.rs`

**Interfaces:**
- Consumes: Task 8 `AsrHandle`/`Command`/`Event`, Task 9 `AppState` helpers, audio `Event::Saved { dir, .. }`.
- Produces: `TranscriptSegment` gains `speaker: Option<asr::transcript::Track>` (`None` for mock lines); `RecorderScreen` gains `asr_job: AsrJob` (`Idle | Waiting | Running { done_secs, total_secs } | Done | Failed(String)`), `transcript_dir: Option<PathBuf>`, `fn set_transcript(&mut self, t: &Transcript)` (maps `Line` to `TranscriptSegment` with `format_clock` timestamps), `fn transcript_empty_state(&self) -> &'static str`. `AppState::on_saved(dir)` sends `Transcribe` when the model is `Ready`, otherwise sets `asr_job = Waiting` with a notice; the Recorder Saved state shows a Transcribe button that does the same for `saved_dir`. `main.rs` wires `AsrHandle::spawn(Arc::new(OrtFactory), AsrConfig::default())` under the same condition as the audio worker (no dev flags, or `--audio`), with the labels from the current language.

- [ ] **Step 1: Write the failing tests**
  - `AppState` with a fake audio event `Saved{dir}` and model `Ready`: a `Transcribe{dir, precision, threads}` is sent (observed by an `AsrHandle` over `FakeFactory` that records commands, then the `Done` event fills `recorder.transcript_segments` with You/Others lines in time order and `asr_job == Done`).
  - Model `Missing` at `Saved`: no `Transcribe` is sent, `asr_job == Waiting`, a notice is queued, `recorder.state` is `Saved` (the recording is untouched); after `Model(Ready)` the Transcribe button path starts the job.
  - Two `Saved` in a row: the second becomes `Queued`; the Recorder shows the first folder's job until it is `Done`.
  - Progress events update `asr_job`; `Cancelled` and `Error` reset it with the message in `last_error`-style banner text (not the red recorder error, since the recording is fine).
  - Starting a new recording while `asr_job` is `Running`: allowed, `RecorderState` goes `Starting` normally.
  - Dev-flag `AppState` (no handles): `transcript_segments` keeps the mock lines and no panic when Saved is simulated.
  - Empty-state text: live mode with no transcript yet gives `The transcript appears after you stop recording`; after `Done` with zero lines it says `No speech found`; Thai entries exist for all new strings (`has_thai`).
  - Search and Copy use the real lines (`transcript_text()` contains `You:` and `Others:` prefixes in live mode).
- [ ] **Step 2: Run** `cargo test` Expected: FAIL.
- [ ] **Step 3: Implement** the right panel (empty state, speaker chips "You" and "Others" next to the timestamp, real lines), the progress line and Cancel button in place of the mock Latency/CPU footer while a job runs, the Transcribe button in the Saved state, and the "Waiting for the model" notice with a button that goes to Settings > Transcription. The header "Live transcript" reads "Transcript" in live mode.
- [ ] **Step 4: Run** `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` Expected: all pass.
- [ ] **Step 5: Manual run** with the real exe: make sure the int8 model is installed (Task 7 helper or the Settings button), record 20 s of Thai speech from the mic with the real app, Stop; the transcript must appear, `transcript.json` and `transcript.txt` must exist next to the WAVs; take screenshots (dark and light, English and Thai) of the empty state, a running job, and the finished transcript; record the measured times in the ledger. If the model is not installed, do the run after Step 5 of Task 11's download and note the order.
- [ ] **Step 6: Commit** `git add src/screens/recorder.rs src/app.rs src/main.rs src/i18n.rs && git commit -m "feat(recorder): transcribe each saved recording and show the transcript"`

### Task 11: Acceptance and documents

**Files:**
- Modify: `TASKS.md`, `ROADMAP.md`, `DECISIONS.md`, `design.md` (section 15), `UI_PROGRESS.md`, `THIRD_PARTY_NOTICES.md`, `src/modals.rs` (About: a line pointing to the notices, if About lists credits), `src/i18n.rs`

**Interfaces:**
- Consumes: the finished feature. No new behavior beyond the About line.

- [ ] **Step 1: Run the acceptance checklist from the spec on this machine and record every result, failures included:** record 2 minutes of Thai speech with the app and Stop; the model downloads with progress from Settings (and a download interrupted by closing the app resumes); the transcript appears on the Recorder with You/Others labels and equals `transcript.txt`; typing `ทำ` in the search box finds a line the model wrote with the split sara am; killing the app during a transcription leaves no `transcript.json`; a 10-minute transcription (record or reuse real audio) gives the measured real-time factor and peak RAM (Task Manager or `Get-Process` working set); recording a new session during a transcription keeps both new WAVs the same length (compare with Python `wave`); starting without the model shows the waiting notice.
- [ ] **Step 2: Update the documents.** `TASKS.md`: move transcription to Completed only for passed items, with evidence; open tasks for failed or untested items and for the live-transcript follow-up. `ROADMAP.md`: tick "Transcription" only if every acceptance item passed. `DECISIONS.md`: entries for the `ort` setup (version, features, build-time download), the model delivery (pinned revision, hashes, download on first use, `ureq` with OS TLS, `zip`, `sha2`), PoodPim port and its MIT notice, segmentation and chunk values, track labels (mic = You, system = Others), the segment-count-instead-of-words call, and the unresolved licence gate. `design.md` section 15: move transcription out of "Not done", add the states and the empty state as implemented behavior, keep "live transcript" and "Transcripts/Recordings real data" listed as not done. `THIRD_PARTY_NOTICES.md`: PoodPim MIT text, OpenThai model licence reference (copy the `LICENSE` file text of the model at the pinned revision), and a plain statement that the licence compatibility check is still open.
- [ ] **Step 3: Verify and commit.** `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` pass; `git add TASKS.md ROADMAP.md DECISIONS.md design.md UI_PROGRESS.md THIRD_PARTY_NOTICES.md src/modals.rs src/i18n.rs && git commit -m "docs: record transcription acceptance results and decisions"`
