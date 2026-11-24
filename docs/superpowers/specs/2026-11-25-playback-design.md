# Playback of real recordings — design

Date: 2026-11-25. Branch `feature/playback` (from `feature/export-transcripts`, PR #4).
Status: implemented (results in `TASKS.md`). The user answered "ทำเลย" to the proposal "Playback next, mixed or one
track at a time?" without choosing, so the author chose the mixed default. Every call below is the author's and is
also a ledger ruling.

## Intent

A meeting recorder that cannot play a recording back makes the user open the folder and use another program to
check what was said. The Recordings screen already has a play button and a player bar, but they are hidden for real
rows because the app has no audio output.

## Goals

- A Play button on each real row plays that recording through the default output device: `mic.wav` and
  `system.wav` mixed into one stream. The same button pauses and resumes.
- The bottom player bar shows the recording, play / pause, elapsed and total time, and a seek bar that can be
  clicked or dragged.
- Playback stops by itself at the end and the bar shows the end; Play again starts from the beginning.
- A recording that has only one track (`mic.wav` or `system.wav`) plays that track.

## Non-goals

- Listening to one track alone, speed control, a volume slider (the Windows volume mixer does that), choosing the
  output device (the default device is used), a waveform, jumping from a transcript line to its time.
- Playing while the Recorder is recording (refused, see below).

## Anti-goals

- Never freeze the window: all file reads and device calls happen on a playback thread.
- Never leave a file open that blocks Delete: deleting the playing recording stops playback first and waits for the
  files to close.
- Never record the playback into a meeting: playing is refused with a notice while a recording is in progress.
- Never play loud garbage: a damaged or unsupported WAV is an error notice, not noise.

## Approaches considered

1. WASAPI shared-mode render through the `wasapi` crate the app already uses for capture, with
   `autoconvert: true` so Windows converts our mono float stream at the file's rate to the device format (chosen):
   no new dependency, no resampler to write, same crate and COM setup as capture.
2. `cpal` or `rodio`: familiar, but a new dependency tree for something the existing crate does.
3. Decode the whole file into memory and play from RAM: simple, but one hour of two tracks is hundreds of MB.
   Rejected; the player streams blocks from disk with the existing `WavReader`.

## Design

**`WavReader::seek_frame(frame)`** (in `asr/wavread.rs`): remembers where the data starts and moves the read
position; used only by playback.

**`src/playback.rs`** (the pure part has no device and no egui):
- `Mixer::open(dir)` opens `mic.wav` and `system.wav` (whichever exist), fails if neither opens or the two rates
  differ, and takes the longest track as the length. `read(max_frames)` returns the tracks summed and clamped to
  [-1, 1]; a shorter track counts as silence after its end; empty at the end. `seek(frame)`, `position()`,
  `total_frames()`, `rate()`.
- `trait Sink`: `start`, `stop`, `discard` (drops what is buffered and returns that many frames), `space`, `padding`,
  `write(&[f32])`, `wait`. The mono block is what a sink gets; the WASAPI sink opens mono then stereo float and
  duplicates samples for stereo, like the capture code.
- `Playback<S: Sink>`: states Paused / Playing / Finished / Failed. `apply(Cmd)` with `Cmd::{Resume, Pause,
  Seek(frame), Quit}` and `tick()`. Pause stops the stream, discards the buffered frames and rewinds the mixer by
  that many, so Resume continues exactly where the listener stopped. Seek does the same flush. `tick()` writes up
  to a tenth of a second into the free space, publishes the position (mixer position minus buffered frames), and
  turns Finished when the mixer is at its end and the buffer has drained. Resume after Finished starts from 0.
  A read error becomes Failed with a message.
- `PlayerHandle::start(dir)` / `start_with(dir, make_sink)`: a thread that opens the mixer and the sink, resumes,
  then loops `wait`, drain commands, `tick`. `snapshot()` gives `{state, position_frames, total_frames, rate,
  error}` from shared atomics; `send(Cmd)`; `stop()` / `Drop` send Quit and join, so the files are closed when it
  returns.
- `WasapiSink` (Windows): shared mode, event driven, `autoconvert: true`, default render device, a 200 ms buffer.

**App.** `AppState` owns `player: Option<PlayerHandle>` and a `player_start` function (replaced by a fake in tests).
Rows send `RecordingsRequest::TogglePlay(dir)`; the bar sends `Seek(fraction)` and `StopPlayback`. Toggling the
folder that is playing pauses or resumes; any other folder replaces the player. Each frame the app copies the
snapshot into the Recordings screen (`PlayerView`) and asks for a repaint every 100 ms while playing. Refusals
and failures use the existing notice: "Stop recording before playing." and "Could not play the recording."
`apply_delete` stops the player first when it is the folder being deleted.

**Screen.** Live rows get a Play / Pause icon button before Open folder. The bottom bar is shown while a player
exists, with the real title, the label "Track: System + Mic Mix", times and the seek bar; the speed chip and volume
icon are not drawn in live mode. Sample rows and the sample bar stay as they are in preview mode.

## Constraints

Rust, egui 0.31, `wasapi` 0.27 (already a dependency), std threads and channels, Windows. No new dependency. Tests
do not assert translated text and never open an audio device (except the `#[ignore]` hardware test). New strings
get a Thai entry and a Thai-test key. Never commit to main; stage by explicit path; no Co-Authored-By.

## Left to the builder

Exact repaint interval, block sizes, button order inside the row, error wording (English plus Thai).
