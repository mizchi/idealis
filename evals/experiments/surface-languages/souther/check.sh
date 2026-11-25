#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "$0")"
compiler="${SOUTHER:-souther}"
out=$(mktemp -d)
trap 'rm -rf -- "$out"' EXIT
"$compiler" compile -d "$out" ./*.sou >"$out/compile.log"
for model in permissions bank order sessions service; do
    test -f "$out/comparison/$model/\$Module.class"
done
