#!/bin/sh
# Plan 0105: the recorded MoonBit bindings are the generator's output after
# moon fmt, and the module has no moon fmt difference.
set -eu
module=tests/rcheck/moonbit
rcheck=crates/rcheck/target/release/rcheck
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/recorded" "$work/fresh"
for copy in recorded fresh; do
  (cd "$module" && find . -name _build -prune -o -name .mooncakes -prune -o -type f -print) |
    while read -r file; do
      mkdir -p "$work/$copy/$(dirname "$file")"
      cp "$module/$file" "$work/$copy/$file"
    done
done
for model in fixtures/rmodel/permissions.model fixtures/rmodel/sessions.model fixtures/rcheck/bank.model; do
  name=$(basename "$model" .model)
  "$rcheck" generate "$model" moonbit > "$work/fresh/$name/bindings.mbt"
done
(cd "$work/recorded" && moon fmt >/dev/null)
(cd "$work/fresh" && moon fmt >/dev/null)
status=0
for name in permissions sessions bank; do
  if ! diff -u "$module/$name/bindings.mbt" "$work/fresh/$name/bindings.mbt"; then
    echo "$module/$name/bindings.mbt is stale: regenerate and run moon fmt" >&2
    status=1
  fi
done
if ! diff -r -x _build -x .mooncakes "$module" "$work/recorded"; then
  echo "$module has moon fmt differences" >&2
  status=1
fi
exit "$status"
