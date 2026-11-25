#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
fizz=$(command -v "${FIZZBEE:-fizz}")
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
cp -- ./*.fizz "$scratch/"
cd "$scratch"

for task in permissions bank order sessions service; do
  if ! "$fizz" --output-dir "$scratch/$task" "$task.fizz" >"$scratch/$task.log" 2>&1 ||
     ! grep -q '^PASSED: Model checker completed successfully' "$scratch/$task.log" ||
     ! grep -q '^Nodes: [0-9]*, queued: 0,' "$scratch/$task.log"; then
    cat "$scratch/$task.log" >&2
    exit 1
  fi
  printf '%s: %s\n' "$task" "$(grep '^Valid Nodes:' "$scratch/$task.log")"
done

# Retain the same model and assertions for the no-fairness control.
sed 's/^atomic fair action Serve:/atomic action Serve:/' service.fizz >"$scratch/service-unfair.fizz"
"$fizz" --output-dir "$scratch/unfair" "$scratch/service-unfair.fizz" >"$scratch/unfair.log" 2>&1 || true
if ! grep -q '^FAILED: Liveness check failed' "$scratch/unfair.log" ||
   ! grep -q '^IsLive: false' "$scratch/unfair.log" ||
   ! grep -q '^Invariant: EventuallyServed' "$scratch/unfair.log"; then
  cat "$scratch/unfair.log" >&2
  exit 1
fi
printf '%s\n' 'service without fairness: liveness counterexample'
