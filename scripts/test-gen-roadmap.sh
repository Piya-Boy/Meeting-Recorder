#!/bin/sh
# Tests for scripts/gen-roadmap.sh. Run: sh scripts/test-gen-roadmap.sh
set -u

HERE=$(cd "$(dirname "$0")" && pwd)
GEN="$HERE/gen-roadmap.sh"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
PASS=0
FAIL=0
CASE=
CASE_FAIL=0
CR=$(printf '\r')

bad() {
  CASE_FAIL=1
  printf 'FAIL %s: %s\n' "$CASE" "$1"
}

assert_eq() { # message expected actual
  [ "$2" = "$3" ] || bad "$1: expected [$2] got [$3]"
}

assert_file_eq() { # message expected_file actual_file
  cmp -s "$2" "$3" || bad "$1: $(diff "$2" "$3" | head -20)"
}

mk() {
  D="$TMP/$CASE"
  rm -rf "$D"
  mkdir -p "$D"
  T="$D/TASKS.md"
  R="$D/ROADMAP.md"
  : > "$T"
}

write_roadmap() {
  printf 'HEAD\n<!-- roadmap:begin -->\nstale\n<!-- roadmap:end -->\nTAIL\n' > "$R"
}

add() { # done|open title "M2 - Core Features"
  box=' '
  [ "$1" = done ] && box=x
  printf -- '- [%s] Task: %s\n  - Milestone: %s\n' "$box" "$2" "$3" >> "$T"
}

run_gen() {
  OUT=$(sh "$GEN" "$@" 2> "$D/err")
  RC=$?
  ERR=$(cat "$D/err")
}

headings() { grep -E '^(### |- \[)' "$R"; }

t_generates_block_and_preserves_outside() {
  mk
  add done 'Setup repo' 'M1 - Foundation'
  add done 'UI screens' 'M2 - Core Features'
  add open 'Audio capture' 'M2 - Core Features'
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'exit code' 0 "$RC"
  cat > "$D/exp" << 'EOF'
HEAD
<!-- roadmap:begin -->
<!-- generated from TASKS.md by scripts/gen-roadmap.sh; do not edit between the markers -->
- Status: M2 - Core Features (In Progress)

## Milestones

### M1 - Foundation (Done)
- [x] Setup repo

### M2 - Core Features (In Progress)
- [x] UI screens
- [ ] Audio capture
<!-- roadmap:end -->
TAIL
EOF
  assert_file_eq 'roadmap' "$D/exp" "$R"
}

t_milestone_status() {
  mk
  add open 'a1' 'M1 - A'
  add open 'a2' 'M1 - A'
  add done 'b1' 'M2 - B'
  add open 'b2' 'M2 - B'
  add done 'c1' 'M3 - C'
  add done 'c2' 'M3 - C'
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'statuses' '### M1 - A (Not Started)
### M2 - B (In Progress)
### M3 - C (Done)' "$(grep '^### ' "$R")"
}

t_current_status_is_first_not_done() {
  mk
  add done 'a1' 'M1 - A'
  add done 'b1' 'M2 - Core Features'
  add open 'b2' 'M2 - Core Features'
  add open 'c1' 'M3 - C'
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'status line' '- Status: M2 - Core Features (In Progress)' "$(grep '^- Status:' "$R")"
}

t_order_numeric_and_done_first() {
  mk
  add open 'ten' 'M10 - Ten'
  add open 'open2' 'M2 - Two'
  add done 'done2' 'M2 - Two'
  add done 'one' 'M1 - One'
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'order' '### M1 - One (Done)
- [x] one
### M2 - Two (In Progress)
- [x] done2
- [ ] open2
### M10 - Ten (Not Started)
- [ ] ten' "$(headings)"
}

t_idempotent() {
  mk
  add done 'a' 'M1 - A'
  add open 'b' 'M2 - B'
  write_roadmap
  run_gen "$T" "$R"
  cp "$R" "$D/first"
  run_gen "$T" "$R"
  assert_eq 'exit code' 0 "$RC"
  assert_file_eq 'second run' "$D/first" "$R"
}

t_check_mode() {
  mk
  add open 'a' 'M1 - A'
  write_roadmap
  cp "$R" "$D/stale"
  run_gen --check "$T" "$R"
  assert_eq 'stale exit code' 1 "$RC"
  assert_file_eq 'stale file unchanged' "$D/stale" "$R"
  run_gen "$T" "$R"
  run_gen --check "$T" "$R"
  assert_eq 'fresh exit code' 0 "$RC"
}

t_missing_milestone_tag_fails() {
  mk
  add open 'tagged' 'M1 - A'
  printf -- '- [ ] Task: Untagged thing\n  - Acceptance Criteria: none\n' >> "$T"
  write_roadmap
  cp "$R" "$D/before"
  run_gen "$T" "$R"
  assert_eq 'exit code' 2 "$RC"
  case "$ERR" in *'Untagged thing'*) ;; *) bad "stderr lacks title: [$ERR]" ;; esac
  assert_file_eq 'roadmap unchanged' "$D/before" "$R"
}

t_marker_errors() {
  mk
  add open 'a' 'M1 - A'
  n=0
  for variant in 'none' 'one' 'two_pairs' 'reversed'; do
    n=$((n + 1))
    case "$variant" in
      none) printf 'HEAD\nTAIL\n' > "$R" ;;
      one) printf 'HEAD\n<!-- roadmap:begin -->\nTAIL\n' > "$R" ;;
      two_pairs) printf '<!-- roadmap:begin -->\n<!-- roadmap:end -->\n<!-- roadmap:begin -->\n<!-- roadmap:end -->\n' > "$R" ;;
      reversed) printf '<!-- roadmap:end -->\nX\n<!-- roadmap:begin -->\n' > "$R" ;;
    esac
    cp "$R" "$D/before$n"
    run_gen "$T" "$R"
    assert_eq "$variant exit code" 2 "$RC"
    assert_file_eq "$variant unchanged" "$D/before$n" "$R"
  done
}

t_special_characters_verbatim() {
  mk
  add open 'Wire A&B \ C | D / E' 'M1 - A'
  write_roadmap
  run_gen "$T" "$R"
  grep -Fxq -- '- [ ] Wire A&B \ C | D / E' "$R" || bad "title mangled: $(cat "$R")"
}

t_crlf_inputs() {
  mk
  add done 'a' 'M1 - A'
  add open 'b' 'M2 - B'
  write_roadmap
  sed "s/\$/$CR/" "$T" > "$D/t.crlf" && mv "$D/t.crlf" "$T"
  sed "s/\$/$CR/" "$R" > "$D/r.crlf" && mv "$D/r.crlf" "$R"
  run_gen "$T" "$R"
  assert_eq 'exit code' 0 "$RC"
  assert_eq 'carriage returns in output' 0 "$(grep -c "$CR" "$R")"
  run_gen --check "$T" "$R"
  assert_eq 'check exit code' 0 "$RC"
}

t_zero_and_all_done() {
  mk
  printf '## Next\n\n_None._\n' > "$T"
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'zero exit' 0 "$RC"
  assert_eq 'zero status' '- Status: No tasks' "$(grep '^- Status:' "$R")"
  assert_eq 'zero headings' 0 "$(grep -c '^### ' "$R")"
  mk
  add done 'a' 'M1 - A'
  add done 'b' 'M2 - B'
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'all done status' '- Status: All milestones Done' "$(grep '^- Status:' "$R")"
}

t_ignores_non_task_lines() {
  mk
  printf '## Rules\n- Something else\n  - Milestone: M9 - X\n- [x] Something without prefix\n## Next\n' > "$T"
  add open 'Real' 'M1 - A'
  write_roadmap
  run_gen "$T" "$R"
  assert_eq 'exit code' 0 "$RC"
  assert_eq 'content' '### M1 - A (Not Started)
- [ ] Real' "$(headings)"
}

t_conflicting_milestone_names_fail() {
  mk
  add open 'a' 'M2 - Alpha'
  add open 'b' 'M2 - Beta'
  write_roadmap
  cp "$R" "$D/before"
  run_gen "$T" "$R"
  assert_eq 'exit code' 2 "$RC"
  assert_file_eq 'roadmap unchanged' "$D/before" "$R"
}

t_hook_stages_roadmap() {
  mk
  G="$D/repo"
  mkdir -p "$G/scripts" "$G/.githooks"
  cp "$GEN" "$G/scripts/gen-roadmap.sh"
  cp "$HERE/../.githooks/pre-commit" "$G/.githooks/pre-commit" 2> /dev/null || :
  git -C "$G" init -q
  git -C "$G" config user.name t
  git -C "$G" config user.email t@t
  git -C "$G" config core.autocrlf false
  git -C "$G" config core.hooksPath .githooks
  printf -- '- [ ] Task: a\n  - Milestone: M1 - A\n' > "$G/TASKS.md"
  printf 'HEAD\n<!-- roadmap:begin -->\n<!-- roadmap:end -->\n' > "$G/ROADMAP.md"
  git -C "$G" add -A
  git -C "$G" commit -q -m first || bad 'first commit failed'
  sed 's/- \[ \] Task: a/- [x] Task: a/' "$G/TASKS.md" > "$G/t.new" && mv "$G/t.new" "$G/TASKS.md"
  git -C "$G" add TASKS.md
  git -C "$G" commit -q -m flip || bad 'flip commit failed'
  case "$(git -C "$G" show --stat --format= HEAD)" in
    *ROADMAP.md*) ;;
    *) bad 'ROADMAP.md not in the commit' ;;
  esac
  case "$(git -C "$G" show HEAD:ROADMAP.md)" in
    *'- [x] a'*) ;;
    *) bad 'committed ROADMAP.md lacks the done task' ;;
  esac
  before=$(git -C "$G" rev-list --count HEAD)
  printf -- '- [ ] Task: untagged\n' >> "$G/TASKS.md"
  git -C "$G" add TASKS.md
  if git -C "$G" commit -q -m bad 2> /dev/null; then bad 'commit with untagged task succeeded'; fi
  assert_eq 'commit count after rejected commit' "$before" "$(git -C "$G" rev-list --count HEAD)"
}

for CASE in $(sed -n 's/^\(t_[a-z_]*\)() {$/\1/p' "$0"); do
  CASE_FAIL=0
  "$CASE"
  if [ "$CASE_FAIL" -eq 0 ]; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); fi
done

printf '%s passed, %s failed\n' "$PASS" "$FAIL"
[ "$FAIL" -eq 0 ]
