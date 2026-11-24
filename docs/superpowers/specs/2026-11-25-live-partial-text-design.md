# Preview text while recording - design

Date: 2026-11-25. Branch `feature/live-partial-text` (on top of `feature/live-transcript`).
Status: the user asked for a delay of about 3 s and chose "partial text" from the options; the calls below are the author's.

## Intent

Final live lines appear only after their segment ends, so with continuous speech the first line came more than 15 s after the speech began. A person in a meeting wants to see words close to when they are said.

## Goals

- Grey preview text for the sentence being spoken, about 3 to 5 s behind the speech, replaced by the final line when its segment is cut.
- The final lines and the final transcript after Stop stay as they are.

## Non-goals

- Word-by-word streaming (the model reads whole segments), preview text in saved files, a separate setting for preview text.

## Approaches

1. Re-read the unfinished tail every 2 s and show it as grey text (chosen): words arrive about 3 to 5 s behind; CPU about doubles; the final lines are as accurate as before.
2. Cut segments every 3 s: cheap, but words are cut in two and read worse.
3. The model's streaming mode: needs another ONNX export and its state; a large change.

## Design

- `SegmentStream::tail()` gives the audio after the last cut and where it starts.
- `LiveTrack::with_partial()` turns on preview reading. In `poll`, after the ready segments are cut, if at least 2 s of audio arrived since the last read, the loud part of the tail (`trim`, at least 0.3 s) is recognized again. A silent tail is not decoded. A cut withdraws the preview. `take_partial_change()` reports only changes (new text, or withdrawn).
- Worker: `Command::LiveStart { partial }`, `Event::LivePartial { dir, track, line: Option<Line> }`, sent after the `LiveLine`s of the same poll so a preview is withdrawn after the line that replaces it.
- App and panel: one preview row per track in time order, grey italics, cleared when live stops or ends, when the final job starts and when the final transcript arrives. "Listening..." shows only while there are no lines and no preview.
- The recorder flushes the WAV header every 1 s (was 5 s) so the audio is readable sooner; a crash loses less.

## Left to the builder

The 2 s pace, the minimum loud length, the wording of the settings text.
