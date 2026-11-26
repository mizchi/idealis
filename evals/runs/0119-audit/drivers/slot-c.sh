#!/bin/bash
# usage: E=<scratch> slot-c.sh <worktree>
# Full-response corpus, temporal parity and the kernel comparison. Each runs alone.
: "${E:?}"
W=$1
cd "$W" || exit 1
R=$E/drivers/run.sh
S=$E/snapshots
PLAN=0119-call-depth-limit
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR NPM_CONFIG_USERCONFIG=/dev/null)
JAVA=$HOME/.local/share/mise/installs/java/temurin-21.0.11+10.0.LTS/bin/java
APJAR=$E/tools/apalache-0.63.0.jar
APTGZ=$E/pinned/apalache-0.63.0.tgz

$R corpus "$W" "${U[@]}" just evaluator-corpus "$S/before-r2" "$S/after" "$JAVA" "$APJAR" "$APTGZ" "$E/runs/corpus" "$PLAN"
$R corpus-summary "$W" "${U[@]}" just evaluator-corpus-summary "$E/runs/corpus" "$E/runs/corpus-summary" "$PLAN"
$R temporal-parity "$W" env IDEALIS_PARITY_PLAN=$PLAN node dist/evals/experiments/temporal-tuning/parity.js "$S/before-parity" "$E/runs/temporal-parity"
$R kernel "$W" "${U[@]}" just evaluator-kernel "$S/before-r2" "$S/after" "$E/runs/kernel"
$R kernel-summary "$W" "${U[@]}" just evaluator-summary "$E/runs/kernel" "$E/runs/kernel-summary" 14 "$PLAN"
echo done > "$E/logs/slot-c.done"
