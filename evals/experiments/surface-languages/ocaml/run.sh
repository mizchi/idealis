#!/bin/sh
# Compile each model against model.ml with warning 8 (non-exhaustive match)
# and the default warnings as errors, then run it. Build output in a temp dir.
set -e
here=$(cd "$(dirname "$0")" && pwd)
out=$(mktemp -d)
status=0
for m in permissions bank order sessions service; do
  cp "$here/model.ml" "$here/$m.ml" "$out/"
  (cd "$out" && ocamlopt -w +a-4-9-40-41-42-44-45-70 -warn-error +a model.ml "$m.ml" -o "$m") || { echo "COMPILE FAILED: $m"; status=1; continue; }
  "$out/$m" || status=1
done
rm -rf "$out"
exit $status
