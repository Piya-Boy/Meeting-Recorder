# Development Workflow

## 1. Understand

- Understand the requested outcome.
- Identify constraints and acceptance criteria.
- Read relevant documentation.

## 2. Explore

- Inspect repository structure.
- Review existing implementation.
- Check Git status.
- Identify dependencies and integration points.

## 3. Plan

- Define implementation approach.
- Break work into features.
- Identify risks.
- Define verification steps.

## 4. Implement

- Work one feature at a time.
- Reuse existing components.
- Avoid unnecessary abstractions.
- Preserve existing behavior.

## 5. Test and Fix

- Run formatter.
- Run linter.
- Run type checks where applicable.
- Run unit and integration tests.
- Build for the target platform.
- Fix discovered problems.

## 6. Review

- Inspect complete Git diff.
- Check for unrelated changes.
- Check error handling and regressions.
- Check for accidental secrets.

## 7. Document

- Update specifications.
- Update TASKS.md. Every task needs a `Milestone:` sub-bullet, for example `  - Milestone: M2 - Core Features`.
- ROADMAP.md milestones are generated from TASKS.md by `scripts/gen-roadmap.sh`. Do not edit the block between the `roadmap:begin` and `roadmap:end` markers by hand.
- Record major decisions in DECISIONS.md.

One-time setup per clone, so the pre-commit hook regenerates and stages ROADMAP.md:

```bash
git config core.hooksPath .githooks
```

Run `sh scripts/test-gen-roadmap.sh` after changing the script or the hook.

## 8. Deliver

- One Feature = One Commit.
- Push after verification when authorized.
- Report actual results.

## 9. Continue

Proceed to the next approved task autonomously.
