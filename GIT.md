# Git Workflow

## Core Rule

One Feature = One Commit + One Push.

## Branch Naming

- feature/feature-name
- bugfix/bug-name
- refactor/component-name
- docs/document-name
- test/test-name
- chore/task-name

Follow existing repository conventions if different.

## Commit Convention

Use Conventional Commits.

Examples:

feat(audio): implement audio capture
feat(transcription): add ASR adapter
fix(recorder): handle device disconnection
refactor(storage): simplify repository
test(audio): add integration tests
docs: update roadmap

## Before Editing

```bash
git status --short
git branch --show-current
git log -5 --oneline
```

## Before Commit

```bash
git status --short
git diff --check
git diff
```

Stage only relevant files.

## Push Rules

- Push only to the intended remote and branch.
- Verify push success.
- Never force-push without explicit approval.
- If push fails, preserve the local commit and report the failure.
