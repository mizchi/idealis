#!/bin/bash
# usage: E=<scratch> slot-d.sh <worktree> <side: oracle-r2|after> <range-high-kind> <range-high-hop>
# Wasm ceilings in three Node modes and in Chromium (main thread and dedicated Worker), then the Go probe.
: "${E:?}"
W=$1; side=$2; kindHigh=$3; hopHigh=$4
cd "$W" || exit 1
S=$E/snapshots
H=$E/stack/target-oracle/release/call-depth-stack-harness
WASM=$S/$side/idealis_core.wasm
CELLS=("hop:1:$hopHigh" "hop-source:1:$hopHigh")
for kind in operator value call membership binding enabled bind-run lazy temporal forward update function action; do CELLS+=("$kind:1:$kindHigh"); done
mkdir -p "$E/runs/wasm"
for mode in default liftoff-only no-liftoff; do
  "$E/drivers/wasm-node.sh" "wasm-$side-node-$mode" "$WASM" "$H" "$E/runs/wasm/$side-node-$mode.json" "$mode" "${CELLS[@]}"
done
"$E/drivers/run.sh" "wasm-$side-chromium" "$W" env NPM_CONFIG_USERCONFIG=/dev/null node dist/evals/experiments/call-depth/browser-ceiling.js "$WASM" "$H" "$E/runs/wasm/$side-chromium.json" "${CELLS[@]}"
mkdir -p "$E/scratch/gorequests-$side"
node dist/evals/experiments/call-depth/go-requests.js "$E/scratch/gorequests-$side" 40 83 84 168 169 1017 1018
(cd evals/experiments/call-depth/go && GOFLAGS=-buildvcs=false mise exec go@1.25.14 -- go run . "$WASM" "$E"/scratch/gorequests-$side/hop-*.json) > "$E/runs/wasm/$side-go.txt" 2>&1
if [ "$side" = oracle-r2 ]; then
  python3 -I "$E/drivers/split-chromium.py" "$E/runs/wasm/$side-chromium.json" "$E/runs/wasm/$side-chromium"
  python3 -I "$E/drivers/ceiling-depths.py" "$H" "node-default=$E/runs/wasm/$side-node-default.json,node-liftoff-only=$E/runs/wasm/$side-node-liftoff-only.json,node-no-liftoff=$E/runs/wasm/$side-node-no-liftoff.json,chromium:main=$E/runs/wasm/$side-chromium-main.json,chromium:worker=$E/runs/wasm/$side-chromium-worker.json" "$E/runs/wasm/$side-depths.json" > "$E/runs/wasm/$side-depths.txt"
fi
echo done > "$E/logs/slot-d-$side.done"
