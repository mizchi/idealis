#!/usr/bin/env bash
# Checks rquint's Alloy lowering of the 0044 models with Alloy 6.2.0
# (plan 0095) and writes each verdict with the SHA-256 of the lowered text.
# SequenceFold does not fit 5-bit integers and is not lowered.
# Usage: alloy.sh <java> <alloy.dist.jar> <output.json>
set -euo pipefail
java=$1 jar=$2 output=$3
root=$(cd "$(dirname "$0")/../../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cargo build --locked --quiet --manifest-path "$root/crates/rquint/Cargo.toml" --example emit_alloy
emit="$root/crates/rquint/target/debug/examples/emit_alloy"
rows=()
for model in Counter SetFold Choice Match NestedFold; do
  for property in safety broken; do
    als="$work/$model-$property.als"
    "$emit" "$root/fixtures/quint-ir/$model.json" "$property" "$als"
    "$java" -jar "$jar" exec --quiet --command '*' --solver sat4j --type json --output "$work/$model-$property" "$als" >/dev/null 2>&1
    found=$(node -e 'const r=require(process.argv[1]);const c=r.commands[process.argv[2]];console.log(c&&(c.solution||[]).length>0?"counterexample":"no-counterexample")' "$work/$model-$property/receipt.json" "c_$property")
    rows+=("{\"name\":\"$model\",\"property\":\"$property\",\"alloy\":\"$found\",\"alsSha256\":\"$(shasum -a 256 "$als" | cut -d' ' -f1)\"}")
  done
done
(IFS=,; printf '{"alloy":"6.2.0","rows":[%s]}\n' "${rows[*]}") >"$output"
