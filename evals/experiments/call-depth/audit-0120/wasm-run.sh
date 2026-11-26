#!/bin/bash
# usage: wasm-run.sh <side> <label> <driver: wasm-ceiling|browser-ceiling> [node flags...]
source <scratch>/env.sh
side=$1; label=$2; driver=$3; shift 3
KINDS="operator value call membership binding enabled bind-run lazy temporal forward capture mixed forward-set forward-function update function action"
if [ "$side" = oracle-after ]; then hop=2000; kind=4000; else hop=300; kind=600; fi
cells=(hop:1:$hop hop-source:1:$hop)
for k in $KINDS; do cells+=("$k:1:$kind"); done
cd $W
exec $S/run --timeout ${WASM_TIMEOUT:-3000} wasm-$side-$label $W env NPM_CONFIG_USERCONFIG=<null-device> node "$@" dist/evals/experiments/call-depth/$driver.js $S/snapshots/$side/idealis_core.wasm $S/snapshots/after/harness-release $S/runs/wasm/$side-$label.json --complete-only --max-work=1000000 "${cells[@]}"
