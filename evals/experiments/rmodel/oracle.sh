#!/usr/bin/env bash
# Checks rmodel's two lowerings of the corpus with the external tools
# (plan 0091): the Alloy text with Alloy 6.2.0 (SAT4J) and each TLA+
# module with TLC. Writes verdicts and the SHA-256 of every lowered file.
# Usage: oracle.sh <java> <alloy.dist.jar> <tla2tools.jar> <output.json>
set -euo pipefail
java=$1 alloy=$2 jar=$3 output=$4
root=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo build --locked --quiet --manifest-path "$root/crates/rmodel/Cargo.toml" --example emit
emit="$root/crates/rmodel/target/debug/examples/emit"
rows=()
for path in "$root"/fixtures/rmodel/*.model; do
  file=$(basename "$path" .model)
  dir="$work/$file"
  "$emit" "$path" "$dir"
  als=$(ls "$dir"/*.als)
  "$java" -jar "$alloy" exec --quiet --command '*' --solver sat4j --type json --output "$dir/alloy" "$als" >"$dir/alloy.log" 2>&1
  for tla in "$dir"/*.tla; do
    module=$(basename "$tla" .tla)
    check=${module#*_}
    label="c_$check"
    found=$(node -e 'const r=require(process.argv[1]);const c=r.commands[process.argv[2]];console.log(c&&(c.solution||[]).length>0?"found":"none")' "$dir/alloy/receipt.json" "$label")
    (cd "$dir" && "$java" -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers 1 -deadlock -config "$module.cfg" "$module.tla" >"$module.log" 2>&1) || true
    if grep -q "Model checking completed. No error has been found" "$dir/$module.log"; then
      tlc=none
    elif grep -q "Invariant $label is violated" "$dir/$module.log"; then
      tlc=found
    else
      tlc=error
    fi
    rows+=("{\"model\":\"$file\",\"check\":\"$check\",\"alloy\":\"$found\",\"tlc\":\"$tlc\",\"alsSha256\":\"$(shasum -a 256 "$als" | cut -d' ' -f1)\",\"tlaSha256\":\"$(shasum -a 256 "$tla" | cut -d' ' -f1)\"}")
  done
done
(IFS=,; printf '{"alloy":"6.2.0","tlc":"%s","rows":[%s]}\n' "$(basename "$jar")" "${rows[*]}") >"$output"
