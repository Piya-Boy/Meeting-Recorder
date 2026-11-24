#!/bin/sh
# Generate the milestone block of ROADMAP.md from the tasks in TASKS.md.
#
# Usage: gen-roadmap.sh [--check] [TASKS_FILE] [ROADMAP_FILE]
#   Defaults to TASKS.md and ROADMAP.md in the repository root.
#   Only the text between the roadmap markers in ROADMAP_FILE is rewritten.
#   --check writes nothing and exits 1 when ROADMAP_FILE is stale.
# Exit codes: 0 ok, 1 --check found a stale file, 2 input error (no file modified).
#
# A task is a line "- [ ] Task: <title>" or "- [x] Task: <title>" followed by an
# indented sub-bullet "  - Milestone: M<number> - <name>".
set -eu

BEGIN_MARK='<!-- roadmap:begin -->'
END_MARK='<!-- roadmap:end -->'

check=0
if [ "${1:-}" = "--check" ]; then
  check=1
  shift
fi
root=$(cd "$(dirname "$0")/.." && pwd)
tasks=${1:-$root/TASKS.md}
roadmap=${2:-$root/ROADMAP.md}

die() {
  printf 'gen-roadmap: %s\n' "$1" >&2
  exit 2
}

[ -f "$tasks" ] || die "missing tasks file: $tasks"
[ -f "$roadmap" ] || die "missing roadmap file: $roadmap"

work=$(mktemp -d)
out=
trap 'rm -rf "$work" "$out"' EXIT

# 1. Build the generated block (markers included) from TASKS.md.
tr -d '\r' < "$tasks" | awk -v begin="$BEGIN_MARK" -v end="$END_MARK" '
function fail(msg) {
  print "gen-roadmap: " msg > "/dev/stderr"
  failed = 1
  exit 2
}
function finish_task() {
  if (!intask) return
  intask = 0
  if (!tag_seen) fail("task has no Milestone tag: " title)
  if (!(tag_id in names)) {
    names[tag_id] = tag_name
    nums[tag_id] = substr(tag_id, 2) + 0
    ids[++count] = tag_id
  } else if (names[tag_id] != tag_name) {
    fail("milestone " tag_id " has conflicting names: [" names[tag_id] "] and [" tag_name "]")
  }
  total[tag_id]++
  if (is_done) {
    done[tag_id]++
    done_items[tag_id, done[tag_id]] = title
  } else {
    open_n[tag_id]++
    open_items[tag_id, open_n[tag_id]] = title
  }
}
function state(id) {
  if (done[id] == total[id]) return "Done"
  if (done[id] == 0) return "Not Started"
  return "In Progress"
}
/^- \[[ xX]\] Task: / {
  finish_task()
  is_done = (substr($0, 4, 1) != " ")
  title = substr($0, index($0, "Task: ") + 6)
  intask = 1
  tag_seen = 0
  next
}
/^[ \t]*[-*+][ \t]+\[.\][ \t]*Task:/ {
  fail("malformed task line (expected \"- [ ] Task: <title>\" at column 0): " $0)
}
intask && /^[ \t]+- Milestone: / {
  if (tag_seen) next
  rest = $0
  sub(/^[ \t]+- Milestone: /, "", rest)
  if (!match(rest, /^M[0-9]+ - /)) fail("bad Milestone tag under task: " title)
  tag_id = substr(rest, 1, RLENGTH - 3)
  tag_name = substr(rest, RLENGTH + 1)
  sub(/[ \t]+$/, "", tag_name)
  if (tag_name == "") fail("empty milestone name under task: " title)
  tag_seen = 1
  next
}
/^[^ \t]/ { finish_task() }
END {
  if (failed) exit 2
  finish_task()
  # Sort milestone ids by their number (insertion sort), ties by id text.
  for (i = 2; i <= count; i++) {
    v = ids[i]
    j = i - 1
    while (j >= 1 && (nums[ids[j]] > nums[v] || (nums[ids[j]] == nums[v] && ids[j] > v))) {
      ids[j + 1] = ids[j]
      j--
    }
    ids[j + 1] = v
  }
  status = "No tasks"
  if (count > 0) status = "All milestones Done"
  for (i = 1; i <= count; i++) {
    if (state(ids[i]) != "Done") {
      status = ids[i] " - " names[ids[i]] " (" state(ids[i]) ")"
      break
    }
  }
  print begin
  print "<!-- generated from TASKS.md by scripts/gen-roadmap.sh; do not edit between the markers -->"
  print "- Status: " status
  print ""
  print "## Milestones"
  for (i = 1; i <= count; i++) {
    id = ids[i]
    print ""
    print "### " id " - " names[id] " (" state(id) ")"
    for (k = 1; k <= done[id]; k++) print "- [x] " done_items[id, k]
    for (k = 1; k <= open_n[id]; k++) print "- [ ] " open_items[id, k]
  }
  print end
}
' > "$work/blk" || exit 2

# 2. Validate the markers in ROADMAP.md (CR-stripped copy).
tr -d '\r' < "$roadmap" > "$work/rm"
begin_n=$(grep -c -x -F -- "$BEGIN_MARK" "$work/rm" || true)
end_n=$(grep -c -x -F -- "$END_MARK" "$work/rm" || true)
[ "$begin_n" -eq 1 ] || die "$roadmap needs exactly one $BEGIN_MARK line (found $begin_n)"
[ "$end_n" -eq 1 ] || die "$roadmap needs exactly one $END_MARK line (found $end_n)"
begin_line=$(grep -n -x -F -- "$BEGIN_MARK" "$work/rm" | cut -d: -f1)
end_line=$(grep -n -x -F -- "$END_MARK" "$work/rm" | cut -d: -f1)
[ "$begin_line" -lt "$end_line" ] || die "$roadmap has the end marker before the begin marker"

# 3. Splice the block between the markers.
out=$(mktemp "$roadmap.XXXXXX")
{
  head -n $((begin_line - 1)) "$work/rm"
  cat "$work/blk"
  tail -n +$((end_line + 1)) "$work/rm"
} > "$out"

if cmp -s "$out" "$work/rm"; then
  exit 0
fi
if [ "$check" -eq 1 ]; then
  printf 'gen-roadmap: %s is stale; run scripts/gen-roadmap.sh\n' "$roadmap" >&2
  exit 1
fi
mv "$out" "$roadmap"
out=
