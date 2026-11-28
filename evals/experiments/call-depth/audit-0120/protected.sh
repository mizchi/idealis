#!/bin/bash
# usage: protected.sh <tree root>; prints one sha256 per protected path (directories hash the sorted list of file digests)
cd "$1" || exit 1
digest() {
  if [ -d "$1" ]; then
    find "$1" -type f -not -path '*/target/*' -not -path '*/node_modules/*' | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256 | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}
for path in crates/rtla/src crates/rtla-compiler/src crates/rtlc/src/ir.rs crates/rtlc/src/safety_parallel.rs crates/rtlc/src/safety_expressions.rs crates/rtlc/src/safety_lazy.rs packages/idealis/src crates/idealis-core/src adapters/go; do
  printf '%s  %s\n' "$(digest "$path")" "$path"
done
