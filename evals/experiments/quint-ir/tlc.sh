#!/usr/bin/env bash
# Checks rquint's TLA+ for the 0044 models with TLC and writes the verdicts
# (plan 0089). Usage: tlc.sh <java> <tla2tools.jar> <output.json>
set -euo pipefail
java=$1 jar=$2 output=$3
root=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo build --locked --quiet --manifest-path "$root/crates/rquint/Cargo.toml" --example emit
emit="$root/crates/rquint/target/debug/examples/emit"
entries=()
for model in Counter SetFold SequenceFold Choice Match NestedFold; do
  for property in safety broken; do
    dir="$work/$model-$property"
    "$emit" "$root/fixtures/quint-ir/$model.json" "$property" "$dir"
    (cd "$dir" && "$java" -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers 1 -deadlock \
      -config "$model.cfg" "$model.tla" >tlc.log 2>&1) || true
    if grep -q "Model checking completed. No error has been found" "$dir/tlc.log"; then
      status=complete
    elif grep -q "Invariant $property is violated" "$dir/tlc.log"; then
      status=invariant-violation
    else
      status=error
    fi
    states=$(grep -oE "[0-9]+ distinct states found" "$dir/tlc.log" | grep -oE "^[0-9]+" | tail -1 || true)
    tla=$(shasum -a 256 "$dir/$model.tla" | cut -d' ' -f1)
    entries+=("{\"name\":\"$model\",\"property\":\"$property\",\"status\":\"$status\",\"states\":${states:-null},\"tlaSha256\":\"$tla\"}")
  done
done
version=$("$java" -cp "$jar" tlc2.TLC -h 2>&1 | grep -oE "Version [^ ]+" | head -1 || true)
(IFS=,; printf '{"tlc":"%s","entries":[%s]}\n' "$version" "${entries[*]}") >"$output"
