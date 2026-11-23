#!/bin/bash
# usage: copy-bench-to-deps.sh <tree> <fresh-executable>; deps does not exist beforehand (checked), copy then compare sha256
set -e
T=$1; src=$2
D=$T/crates/rtlc/target/release/deps
[ -e "$D" ] && { echo "deps already exists: $D"; ls -la "$D"; exit 3; }
mkdir -p "$D"
cp "$src" "$D/$(basename "$src")"
a=$(shasum -a 256 < "$src" | cut -d' ' -f1); b=$(shasum -a 256 < "$D/$(basename "$src")" | cut -d' ' -f1)
echo "fresh=$a copy=$b"
[ "$a" = "$b" ]
