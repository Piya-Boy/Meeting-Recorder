# Testing Standards

## Principles

- Test observable behavior.
- Add regression tests for bug fixes.
- Keep tests deterministic.
- Do not delete tests to force success.
- Distinguish automated and manual verification.

## Checklist

- [ ] Formatter passes.
- [ ] Linter passes.
- [ ] Type checking passes when applicable.
- [ ] Unit tests pass.
- [ ] Integration tests pass.
- [ ] Build succeeds.
- [ ] Manual verification completed where needed.
- [ ] Error paths checked.
- [ ] Git diff reviewed.

## Reporting

Report exact commands and actual results.

If a check was skipped, explain why.
Never claim an unexecuted check passed.
