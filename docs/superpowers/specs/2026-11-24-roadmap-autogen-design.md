# ROADMAP auto-generation from TASKS.md

Status: approved by user 2026-11-24.

## Intent

ROADMAP.md drifts from TASKS.md because both are edited by hand. Make TASKS.md the single source of truth and generate the milestone list in ROADMAP.md from it.

## Goals

- Milestone checklists and milestone status in ROADMAP.md are derived from TASKS.md.
- Generation runs automatically on every commit (human or agent).
- No new dependencies.

## Non-goals

- Generating the ROADMAP header text (Goal, Exit Criteria) or the Status Values section. These stay hand-written.
- CI integration (a `--check` mode is provided for later use).

## Design

- Each task in TASKS.md gets a sub-bullet `  - Milestone: <id> - <name>` (for example `M2 - Core Features`).
- ROADMAP.md keeps hand-written text outside `<!-- roadmap:begin -->` / `<!-- roadmap:end -->`. Script rewrites only the block between the markers.
- Per milestone: `[x]` when the task is under Completed, `[ ]` otherwise. Status: all done = Done, some done = In Progress, none = Not Started.
- "Current Milestone" status line: first milestone that is not Done.
- `scripts/gen-roadmap.sh`: sh + awk (ships with Git for Windows). Flags: none = rewrite, `--check` = exit 1 if ROADMAP.md is stale.
- `.githooks/pre-commit`: runs the script; if ROADMAP.md changed, `git add ROADMAP.md`.
- Enable once per clone: `git config core.hooksPath .githooks`. Documented in WORKFLOW.md step 7 (not README, to avoid touching the in-progress license edits).
- Migration: ROADMAP items with no task in TASKS.md (all of M1, audio, ASR, SQLite, exports, M3 items) become tasks with milestone tags.

## Approaches considered

1. Commit gate hook that blocks until ROADMAP is hand-updated. Enforces but does not automate.
2. Generate from TASKS.md. Chosen by user: one source of truth, fully mechanical.
3. Reminder-only Stop hook / CLAUDE.md rule. Not enforced.

## Known limits (builder's calls)

- Script reads TASKS.md from the working tree, not the index. Partially staged TASKS.md can yield a ROADMAP built from unstaged text.
- Hook is opt-in per clone via `core.hooksPath`.
- Branch: `chore/roadmap-autogen`. Commit: `chore: generate ROADMAP.md from TASKS.md`.
