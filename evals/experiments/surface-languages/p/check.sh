#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
p=$(command -v "${P_CLI:-p}")
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT

compile() {
  local source=$1 output=$2
  if ! "$p" compile --pfiles "$source" --projname Surface       --outdir "$output" >"$output.build.log" 2>&1; then
    cat "$output.build.log" >&2
    return 1
  fi
}

check() {
  local task=$1 output=$2
  "$p" check "$output/PChecker/net8.0/Surface.dll" --testcase "$task" \
    --seed 110 --schedules 256 --max-steps 256 --fail-on-maxsteps \
    --outdir "$output/check" >"$output.check.log" 2>&1
}

for task in permissions bank order sessions service; do
  output="$scratch/$task"
  compile "$task.p" "$output"
  if ! check "$task" "$output" ||
      ! grep -qE 'Found 0 bugs\.$' "$output.check.log" ||
      ! grep -qE 'Explored 256 schedules$' "$output.check.log"; then
    cat "$output.check.log" >&2
    exit 1
  fi
  printf '%s: 256 schedules, 0 bugs\n' "$task"
done

# Omit the final drain while leaving the service and monitor otherwise identical.
undrained="$scratch/undrained"
sed 's/while (sizeof(waiting) > 0)/while (false)/' service.p >"$undrained.p"
compile "$undrained.p" "$undrained"
status=0
check service "$undrained" || status=$?
if [[ $status != 1 ]] ||
    ! grep -qF "EventuallyServed detected liveness bug in hot state 'Waiting'" \
      "$undrained"/check/BugFinding/Surface_0_0.txt; then
  cat "$undrained.check.log" >&2
  exit 1
fi
printf '%s\n' 'service without the drain: EventuallyServed counterexample'
