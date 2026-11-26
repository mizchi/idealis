#!/bin/bash
# usage: copy-bench-into-deps.sh <tree> <fresh-executable>
# Copies the bench executable into target/release/deps (which may exist) and compares sha256.
set -e
T=$1; src=$2
D=$T/crates/rtlc/target/release/deps
mkdir -p "$D"
if ls "$D" | grep -qE '^safety-[a-f0-9]+$'; then echo "a safety bench is already in $D"; ls -la "$D" | grep safety; exit 3; fi
cp "$src" "$D/$(basename "$src")"
a=$(shasum -a 256 < "$src" | cut -d' ' -f1); b=$(shasum -a 256 < "$D/$(basename "$src")" | cut -d' ' -f1)
echo "fresh=$a copy=$b"
[ "$a" = "$b" ]
