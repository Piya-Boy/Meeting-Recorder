# Transcription language and word fixes - design

Date: 2026-11-25. Branch `feature/language-and-glossary` (on top of `feature/live-partial-text`).
Status: the user asked for the most accurate transcripts, saw the measurements (Thai + English prompt, beam search, hotwords) and chose "1 + 2": a Thai + English option and word fixes. The calls below are the author's.

## Intent

English words in Thai speech came out in Thai letters, and terms the model does not know came out wrong in the same way every time. The user wants them right.

## Goals

- A Language setting: Thai (prompt 32, the default) or Thai + English (prompt 63, English words in Latin letters).
- A word-fix list the user writes, applied to every line the app shows or saves from now on.

## Non-goals

- Beam search and hotwords (measured, not worth it: `DECISIONS.md`); changing transcripts made earlier; fuzzy matching.

## Design

- `Language` in `asr::engine`, `Recognizer::set_language` (no-op by default); the ONNX recognizer sends the prompt with each segment.
- `asr::glossary::Glossary`: rules from text, one a line: `wrong, wrong = right` (split at the last `=`), blank and `#` lines skipped, an empty right side deletes. Applied left to right in one pass, longest wrong form first, Latin letters in any case, no chaining. `invalid_lines` counts lines that are not rules.
- `asr::worker::TextOptions { language, glossary }` rides on `Command::Transcribe` and `Command::LiveStart`. The worker sets the language on the recognizer before a job or a live run and applies the fixes to final lines (a line left empty is dropped), live lines and previews.
- Settings > Transcription: a Language select and a Word fixes box with a summary; saved in `settings.ini` as `asr.language=th|th-en` and one `asr.glossary=` line per rule.

## Left to the builder

The wording, where the box sits on the page, the summary format.
