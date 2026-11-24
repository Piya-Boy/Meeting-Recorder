# Audio capture (WASAPI, dual track) design

Status: implemented on branch `feature/audio-capture` (PR #2). Acceptance on a real Zoom/Teams call is still open; see `TASKS.md`.

## Intent and why

The Recorder screen is a UI mock: Record, Pause and Stop only change `RecorderState`, and the timer, levels and device lists are fake. The user records meetings held in Zoom or Microsoft Teams on this Windows machine and wants the real thing: the other participants' voices and the user's own voice, saved as two separate WAV files that survive a crash and stay aligned. This spec covers capture and file writing only. Transcription, voice-activity detection, the SQLite index and exports are separate specs.

## Goals

- Record, Pause and Stop produce two mono WAV files: `system.wav` (the other participants) and `mic.wav` (the user).
- System track captures only Zoom or Teams audio (process loopback), with a whole-system fallback.
- Each source has its own visible status; one failing never silently stops the other.
- Both files have the same length (within the tolerance below) and the same time origin.
- A crash or power loss leaves playable files.
- The UI thread never blocks on audio or file work (design.md 7.5).
- Live input levels for the existing level meters.

## Non-goals (this spec)

- Transcription, resampling to 16 kHz, and silence/voice detection. These belong to the ASR spec.
- SQLite index, the Recordings and Transcripts lists reading real data, exports.
- Speaker separation inside the system track (design.md section 12 excludes it).
- Settings channel modes "Stereo mixed" and "Mono downmix" (mixing can happen at playback or export). The dropdown stays as is until the storage or export spec removes or implements it.
- The "Test system audio" and "Test microphone" buttons, and the folder picker (they stay UI-only toasts).
- A UI to edit the list of meeting-app process names.

## Anti-goals (failure even if it technically works)

- Silent data loss: a source that dies and nobody can tell.
- A hidden fallback: capturing the whole system's sound without a clearly visible indication.
- Tracks of different length or drifting apart, which would break later transcript timestamps.
- Any audio, file or device call on the egui update thread.

## Constraints

- Windows only. Capture code lives behind `cfg(windows)`, like the existing DWM code.
- Rust, no async runtime. Use the `wasapi` crate (HEnquist) unless it turns out unable to do something below; raw `windows`/`windows-sys` COM is the escape hatch. Method names seen in docs (`new_application_loopback_client(process_id, include_tree)`, and a possibly different `ProcessAudioClient::new`) are unverified: the builder checks them against the pinned version.
- Per-process loopback needs a recent Windows build (the user's machine is Windows 11); the exact minimum build is for the builder to verify. On a machine without support the system source falls back to whole-system capture.
- Files: PCM 16-bit mono WAV at the Settings sample rate (default 48,000 Hz; options 44,100 and 16,000).
- No new settings file; reuse `settings_store.rs` conventions (plain key=value, atomic write, silent failure).
- User-visible text goes through `i18n::tr` with Thai entries (`src/i18n.rs`).

## Approaches considered

1. **`wasapi` crate, one thread per source, std channels to the UI (chosen).** Supports shared-mode capture, device loopback and per-process loopback. WASAPI event handles are blocking, so plain threads fit; no async runtime added.
2. **`cpal`.** Simpler and cross-platform, but cannot capture a single process, which the user chose. Rejected.
3. **Raw COM through the `windows` crate.** Full control, much more `unsafe` and boilerplate. Kept only as a fallback if the `wasapi` crate lacks something.
4. **One stereo file with both sources.** Simpler files, but loses source separation the user wants for transcription and review. Rejected.

## Decisions made with the user

- System track: capture only Zoom and Teams (process and its child processes), with whole-system capture as the fallback.
- Fallback rules: if no Zoom or Teams process exists when Record is pressed, or the meeting runs in a browser (a browser's tabs cannot be isolated), capture the whole system and show a clear badge "Capturing all system audio". If the target app exits or restarts mid-recording, keep recording the microphone, poll for the app every 2 seconds, re-attach the system track when it returns, and show a notice. The time the app was gone is silence in `system.wav`.
- Pause discards audio: the paused time is not in either file, both tracks stop and resume together, and the timer and later transcript times count recorded time only.
- Assumes the user wears headphones. With speakers the other participants leak into the microphone and the transcripts would duplicate text. Not handled in this spec.
- Silence trimming for transcription is a separate, later concern (own energy-based VAD behind one interface, Silero as a swap). Raw audio is stored untrimmed.

## Design (my calls, for the user to check)

### Files

```
<recording dir>\<YYYY-MM-DD_HHMM>_<name>\
  system.wav
  mic.wav
```

`<recording dir>` is the Recorder screen's `destination` field; `<name>` comes from its `file_name` field, with characters invalid in Windows file names replaced, and `-2`, `-3` appended if the folder exists. On Stop the worker reports the folder path; registering it in the Recordings list is a later spec.

### Crash safety

WAV stores the data size in its header. The writer updates the header sizes and flushes every second (every 5 seconds until 2026-11-25) and on Stop. Opening a file whose header says less than the file holds (crash) must still work: the reader or a repair helper trusts the file length. A recording that reaches the WAV size limit (about 11 hours at 48 kHz mono 16-bit) is finalized and the user is warned.

### Sync

Both tracks start from one shared start instant. Each buffer from WASAPI carries a device or QPC timestamp. The writer keeps the number of samples written; when a buffer starts later than the samples written so far (loopback sends nothing while no sound plays, device glitches, a missing source), it inserts zeros to cover the gap, so each file length always equals recorded time. Drift between the two device clocks is corrected by the same timestamps (drop or insert the few samples needed). Target: at Stop the two files differ by at most 100 ms after one hour. This number is my call; the builder may tighten it.

### Threads and messages

```
UI thread --Command--> session worker --controls--> system thread + mic thread
UI thread <--Event---- session worker <--buffers--- (each: WASAPI -> level meter -> WAV writer)
```

- `Command`: `Start { sources, sample_rate, dir, name }`, `Pause`, `Resume`, `Stop`.
- `Event`: `State(SessionState)`, `Levels { system: f32, mic: f32 }` (about every 50 ms, peak or RMS from the capture thread), `SourceStatus { source, status }` (`Active`, `FallbackAllSystem`, `Waiting for app`, `Device lost`, `Disabled`), `Elapsed(Duration)`, `Saved { dir, duration }`, `Error(message)`.
- The UI polls the receiver once per frame and calls `request_repaint_after` while a session is active.

### States

`Idle -> Starting -> Recording <-> Paused -> Stopping -> Finalizing -> Saved`, with `Error` reachable from Starting and Recording (design.md section 8). This adds Starting, Stopping, Finalizing and Saved to the existing `RecorderState` (Idle, Recording, Paused, Processing, Error); `Processing` is renamed or split by the builder, and the unit tests and shortcuts keep working.

During Pause the capture streams keep running and the writers discard data, so resume is instant and the meters keep moving (builder may choose differently if it causes trouble).

### Source rules

- Both sources disabled: Start is refused with a message.
- One source fails to start: start the other, show the warning, recording continues.
- Neither starts: `Error`, no files left behind.
- A device disappears mid-recording: keep the other source, mark this one `Device lost`, fill its track with silence, retry the device every 2 seconds, and re-attach it when it returns.
- A device change in Settings while recording does not affect the running session (design.md section 5.6).
- Devices are stored by device ID in `settings.ini` (`audio.system_device`, `audio.mic_device`; empty means the Windows default), not by list index. The Settings dropdowns list the real devices.

### Meeting app detection

A fixed list of process names: `Zoom.exe`, `ms-teams.exe`, `Teams.exe`. Matching is by executable name through a process snapshot (`windows-sys` Toolhelp feature). The target is the lowest-level matching process whose parent is not itself a match, and capture includes its process tree.

## Testing and acceptance

Automated (no audio hardware needed):
- WAV writer: header sizes correct after Stop; header updated mid-recording; a truncated file stays readable.
- Silence-fill and sync arithmetic: given buffers with timestamps, sample counts match elapsed time; drift correction stays within tolerance.
- State machine: legal and illegal transitions, Pause then Stop, error paths.
- Process finder with a fake process list (nested Teams processes, no match, two matches).
- Folder naming: invalid characters, collisions.

Manual, on this machine (an ignored integration test is optional):
- Record a 2-minute Zoom or Teams call: both WAVs play in an ordinary player, the other participants are only in `system.wav`, the user only in `mic.wav`.
- Lengths differ by at most 100 ms. Pause for 30 seconds: files do not contain the gap.
- Kill the app mid-recording: both files still play.
- Start without Zoom or Teams running: badge shows all-system capture.
- Quit Zoom mid-recording and reopen it: the system track resumes.
- Unplug the microphone mid-recording: recording continues with the system track and the UI says so.

The UI stays responsive (no frame stalls) throughout.

## Left to the builder

Crate version and exact API; buffer sizes and event timeouts; level meter smoothing; the exact Thai and English message wording; how the Recorder screen's mock fields (`sys_audio_level`, `mic_level`, device lists, timer) map to events; how to keep `RecorderState` helpers and tests working.

## Open items (not blocking)

- Wiring Settings > Storage's folder picker and persisting the recording folder.
- A consent notice for recording other people (a product and legal question, not decided).
- Speaker output instead of headphones (echo handling).
