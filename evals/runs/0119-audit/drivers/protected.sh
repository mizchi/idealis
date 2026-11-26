#!/bin/bash
# usage: protected.sh <tree> : prints one sha256 per protected path set and a combined digest
T=$1
cd "$T" || exit 1
for set in crates/rtla/src crates/rtla-compiler/src crates/rtlc/src/ir.rs packages/idealis/src crates/idealis-core/src adapters/go; do
  digest=$(find "$set" -type f | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256 | cut -d' ' -f1)
  echo "$digest  $set"
done
