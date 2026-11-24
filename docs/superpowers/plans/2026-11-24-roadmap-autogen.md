# ROADMAP Auto-generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate the milestone list and status in `ROADMAP.md` from `TASKS.md`, and run that generation automatically on every commit.

**Architecture:** One POSIX `sh` + `awk` script reads task lines and `Milestone:` sub-bullets from `TASKS.md` and rewrites only the region of `ROADMAP.md` between two HTML-comment markers. A tracked `pre-commit` hook runs the script and stages `ROADMAP.md` when it changed. A plain-`sh` test script covers both.

**Tech Stack:** POSIX sh, awk (both ship with Git for Windows), git hooks via `core.hooksPath`. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-11-24-roadmap-autogen-design.md`

## Global Constraints

- No new dependencies; only `sh`, `awk`, `git`, and coreutils available in Git for Windows.
- Only the text between `<!-- roadmap:begin -->` and `<!-- roadmap:end -->` in `ROADMAP.md` is generated; everything outside is untouched.
- Task tag format in `TASKS.md`: a sub-bullet `  - Milestone: <id> - <name>`, for example `  - Milestone: M2 - Core Features`.
- Milestone status: all tasks `[x]` = `Done`, some = `In Progress`, none = `Not Started`.
- Script exit codes: `0` ok, `1` `--check` found `ROADMAP.md` stale, `2` input error. On exit `2` no file is modified.
- Repo rule (CLAUDE.md): One Feature = One Commit + One Push. Tasks 1 to 3 below are one feature: make a single commit at the end of Task 3, no intermediate commits.
- Commit message: `chore: generate ROADMAP.md from TASKS.md`, Conventional Commits, no `Co-Authored-By` and no "Generated with" lines (user's global rule overrides the session attribution reminder).
- Branch is `chore/roadmap-autogen`. The working tree has unrelated uncommitted edits to `Cargo.toml`, `LICENSE`, `README.md`: never stage them. Stage files by explicit path only.
- Do not edit `README.md` (it has uncommitted license edits). Hook setup is documented in `WORKFLOW.md`.

## Review Focus

- `TASKS.md` or `ROADMAP.md` with CRLF line endings (Windows `autocrlf`): output has no `\r`, and `--check` is not falsely stale.
- A task line with no `Milestone:` tag: script fails with exit 2 and a message naming the task; it is never silently dropped from the roadmap.
- Task titles containing `&`, `\`, `|`, or `/`: copied verbatim into `ROADMAP.md`.
- `ROADMAP.md` with a missing, duplicated, or reversed marker pair: exit 2, file untouched.
- Zero tasks, or every task done: `Status` line reads `No tasks` or `All milestones Done`, not an empty or misleading milestone.

---

### Task 1: Generator script

**Files:**
- Create: `scripts/gen-roadmap.sh`
- Create: `scripts/test-gen-roadmap.sh`

**Interfaces:**
- Produces: `sh scripts/gen-roadmap.sh [--check] [TASKS_FILE] [ROADMAP_FILE]`. Defaults to `TASKS.md` and `ROADMAP.md` in the repo root (the parent of the script's directory). Rewrites `ROADMAP_FILE` in place; with `--check` it writes nothing and exits 1 when the file would change. Prints errors to stderr.
- Produces: generated block layout, exactly:

```
<!-- roadmap:begin -->
<!-- generated from TASKS.md by scripts/gen-roadmap.sh; do not edit between the markers -->
- Status: M2 - Core Features (In Progress)

## Milestones

### M1 - Foundation (Done)
- [x] <task title>

### M2 - Core Features (In Progress)
- [x] <done task title>
- [ ] <open task title>
<!-- roadmap:end -->
```

  Rules: milestones sorted by the number in the id; inside a milestone, done tasks first then open tasks, each group in `TASKS.md` order; `Status` is the first milestone that is not Done, or `All milestones Done`, or `No tasks` when there are none. A blank line separates milestones; no blank line before `<!-- roadmap:end -->`.
- Produces: `scripts/test-gen-roadmap.sh`, a runnable test script (`sh scripts/test-gen-roadmap.sh`, exit 0 = all pass) that builds fixtures in a `mktemp -d` directory. Task 2 appends a hook case to it.

- [ ] **Step 1: Write the failing tests in `scripts/test-gen-roadmap.sh`**

Harness: small `assert_eq`/`assert_file_eq` helpers, a `run_gen` wrapper capturing exit code, and one function per case. Each case writes a fixture `TASKS.md` and a `ROADMAP.md` containing `HEAD`, the two markers around a stale line, and `TAIL`, then asserts. Cases:

  - `t_generates_block_and_preserves_outside`: two milestones (M1 all done, M2 one done + one open) produce exactly the layout above; `HEAD` and `TAIL` lines are byte-identical afterwards.
  - `t_milestone_status`: a milestone with 0/1/2 of 2 done shows `Not Started` / `In Progress` / `Done`.
  - `t_current_status_is_first_not_done`: M1 Done, M2 In Progress, M3 Not Started gives `- Status: M2 - Core Features (In Progress)`.
  - `t_order_numeric_and_done_first`: tasks tagged `M10`, `M2`, `M1` in that file order list as M1, M2, M10; an open task listed before a done task in `TASKS.md` prints after it.
  - `t_idempotent`: second run leaves `ROADMAP.md` byte-identical.
  - `t_check_mode`: `--check` exits 1 on a stale file and leaves it unchanged; after a normal run `--check` exits 0.
  - `t_missing_milestone_tag_fails`: a task without a tag exits 2, stderr contains the task title, `ROADMAP.md` unchanged.
  - `t_marker_errors`: no markers, one marker, two pairs, and end-before-begin each exit 2 with `ROADMAP.md` unchanged.
  - `t_special_characters_verbatim`: title `Wire A&B \ C | D / E` appears verbatim in the output.
  - `t_crlf_inputs`: `TASKS.md` and `ROADMAP.md` written with `\r\n`: output contains no `\r`; `--check` after a normal run exits 0.
  - `t_zero_and_all_done`: no task lines gives `- Status: No tasks` and no `### ` lines; all done gives `- Status: All milestones Done`.
  - `t_ignores_non_task_lines`: a `  - Milestone: M9 - X` sub-bullet under a non-task bullet, and `- [x] Something` lines without the `Task:` prefix, are ignored.
  - `t_conflicting_milestone_names_fail`: same id `M2` with two different names exits 2.

The script ends by printing `N passed, M failed` and exits non-zero on any failure.

- [ ] **Step 2: Run tests to verify they fail**

Run: `sh scripts/test-gen-roadmap.sh`
Expected: FAIL; every case fails because `scripts/gen-roadmap.sh` does not exist.

- [ ] **Step 3: Implement `scripts/gen-roadmap.sh`**

`#!/bin/sh`, `set -eu`. Parse args (`--check`, then optional positional files). Strip `\r` from both inputs. One awk pass over `TASKS.md` matches task lines with `^- \[( |x)\] Task: (.*)$` and, on the most recent task, a tag line `^[ \t]+- Milestone: (M[0-9]+) - (.+)$`; a task line with no tag before the next task line or EOF is an error. Build the block in awk (print with `printf "%s"`, never `sub`/`gsub` replacement strings, so `&` and `\` stay literal), then splice it into `ROADMAP.md` between markers (validate exactly one begin, one end, begin before end first). Write to a temp file in the same directory and `mv` over the target; compare with `cmp` for `--check`. Run `shellcheck`-clean POSIX only (no bashisms, no `sort -V`; sort milestones by the numeric part inside awk).

- [ ] **Step 4: Run tests to verify they pass**

Run: `sh scripts/test-gen-roadmap.sh`
Expected: PASS, output ends with `13 passed, 0 failed`.

---

### Task 2: pre-commit hook

**Files:**
- Create: `.githooks/pre-commit`
- Modify: `scripts/test-gen-roadmap.sh` (append one case)

**Interfaces:**
- Consumes: `sh scripts/gen-roadmap.sh` exit codes from Task 1.
- Produces: `.githooks/pre-commit`, a `#!/bin/sh` hook run from the repo root: if `TASKS.md` and `ROADMAP.md` both exist, run the generator, abort the commit (exit 1, message on stderr) when it exits non-zero, then `git add ROADMAP.md` if the file differs from the index. File mode is executable in the index.

- [ ] **Step 1: Write the failing test `t_hook_stages_roadmap`**

In a `mktemp -d` repo: `git init`, copy `scripts/gen-roadmap.sh` and `.githooks/pre-commit`, write minimal `TASKS.md` and `ROADMAP.md` with markers, `git config core.hooksPath .githooks`, `git config user.name t` / `user.email t@t`, commit once, then flip a task from `[ ]` to `[x]` in `TASKS.md`, `git add TASKS.md` only, `git commit -m x`. Assert `git show --stat HEAD` lists `ROADMAP.md` and that the committed `ROADMAP.md` shows the flipped task as `- [x]`. Add a second assertion: a task with no tag makes `git commit` exit non-zero and `git log --oneline | wc -l` stays at 1.

- [ ] **Step 2: Run test to verify it fails**

Run: `sh scripts/test-gen-roadmap.sh`
Expected: FAIL on `t_hook_stages_roadmap` (hook file missing, `ROADMAP.md` not in the commit).

- [ ] **Step 3: Implement `.githooks/pre-commit`**

Behavior as in Interfaces. Then run `git update-index --add --chmod=+x .githooks/pre-commit` so the exec bit is recorded for non-Windows clones.

- [ ] **Step 4: Run tests to verify they pass**

Run: `sh scripts/test-gen-roadmap.sh`
Expected: PASS, output ends with `14 passed, 0 failed`.

---

### Task 3: Migrate data, document, enable, commit

**Files:**
- Modify: `TASKS.md`, `ROADMAP.md`, `WORKFLOW.md`, `DECISIONS.md`
- Add to commit: `docs/superpowers/specs/2026-11-24-roadmap-autogen-design.md`, this plan

**Interfaces:**
- Consumes: `scripts/gen-roadmap.sh`, `.githooks/pre-commit`.

- [ ] **Step 1: Tag every task in `TASKS.md` with its milestone**

Add a `  - Milestone: <id> - <name>` sub-bullet directly under each task line:
  - `M2 - Core Features`: all 12 existing Completed tasks, and the open task "Wire the UI to real backends ...".
  - `M3 - Stabilization`: the open task "Keyboard navigation inside tables and dropdown popups".

- [ ] **Step 2: Add the roadmap items that have no task yet**

Add as new tasks (each with title, one-line outcome, milestone tag):
  - Open, `M2 - Core Features`: "Audio capture (Windows WASAPI, dual track)", "Transcription (OpenThai ASR on ONNX Runtime)", "Storage (SQLite index, WAV / transcript files)", "Exports (TXT, SRT, JSON)". Put under `## Next`.
  - Open, `M3 - Stabilization`: "Error handling", "Performance review", "Security review", "Release validation (installer, release-build check)". Under `## Next`.
  - Done, `M1 - Foundation`: "Repository setup (Rust + eframe/egui 0.31, no WebView)", "Architecture (screens / components / shell / theme / i18n / branding)", "Build and test pipeline (fmt, clippy, test pass; 19 tests)". Under `## Completed`, with `Verification Evidence: carried over from the previous ROADMAP.md; not re-verified in this change.` and no commit field.

  The coarse ROADMAP bullets "UI: 10 screens ...", "UI: dark and light themes ...", "UI: app logo ..." and "Persist settings" are already covered by existing Completed tasks; do not duplicate them.

- [ ] **Step 3: Restructure `ROADMAP.md`**

Keep `## Current Milestone` with the hand-written `Goal:` and `Exit Criteria:` lines. Remove the hand-written `Status:` line and the whole `## Milestones` section. Insert `<!-- roadmap:begin -->` and `<!-- roadmap:end -->` after `Exit Criteria` and before `## Status Values`, then run `sh scripts/gen-roadmap.sh`. Keep the closing sentence about synchronization but reword it to say `ROADMAP.md` milestones are generated from `TASKS.md`; edit `TASKS.md`, not the generated block.

- [ ] **Step 4: Document in `WORKFLOW.md` and `DECISIONS.md`**

`WORKFLOW.md` step 7: replace "Update ROADMAP.md." with "Update TASKS.md (every task needs a `Milestone:` sub-bullet); ROADMAP.md is generated from it by `scripts/gen-roadmap.sh`." Add a short "One-time setup" note: `git config core.hooksPath .githooks`. `DECISIONS.md`: add an entry (template format) dated 2026-11-24, "ROADMAP.md generated from TASKS.md", status Accepted, with the alternatives and the working-tree-vs-index limit from the spec.

- [ ] **Step 5: Enable the hook locally and verify**

Run: `git config core.hooksPath .githooks`
Run: `sh scripts/gen-roadmap.sh --check; echo $?`
Expected: `0`.
Run: `sh scripts/test-gen-roadmap.sh`
Expected: `14 passed, 0 failed`.
Open `ROADMAP.md`: M1 Done, M2 In Progress with the 12 done tasks first, M3 Not Started; Status line `M2 - Core Features (In Progress)`.

- [ ] **Step 6: Review the diff, then commit once**

Run: `git status --short` and `git diff` for the listed files only; confirm `Cargo.toml`, `LICENSE`, `README.md` are not staged.

```bash
git add scripts/gen-roadmap.sh scripts/test-gen-roadmap.sh .githooks/pre-commit TASKS.md ROADMAP.md WORKFLOW.md DECISIONS.md docs/superpowers
git commit -m "chore: generate ROADMAP.md from TASKS.md"
```

The pre-commit hook runs on this commit: confirm it exits cleanly and `git show --stat HEAD` lists no unintended files. Push only when the user authorizes it.
