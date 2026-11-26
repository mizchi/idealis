#!/bin/bash
# usage: E=<scratch> slot-b.sh <worktree>
# Runs the TS contracts, the 4-path comparison, and the TLC comparison.
: "${E:?}"
W=$1
cd "$W" || exit 1
R=$E/drivers/run.sh
S=$E/snapshots
CASES=$W/fixtures/nesting-chains/cases.json,$W/fixtures/nesting-chains/cases-0075.json,$W/fixtures/call-depth/cases-0119.json
QUINT=$W/fixtures/nesting-chains/quint
mkdir -p "$E/runs/paths" "$E/runs/tlc"

$R green-ts-call-depth "$W" env -u CLAUDECODE node --test dist/tests/call-depth/contract-test.js
$R green-ts-nesting-chains "$W" env -u CLAUDECODE node --test dist/tests/nesting-chains/contract-test.js

$R paths-before "$W" node "$E/drivers/paths.mjs" before "$S/before-r2" "$E/before/dist" "$W/dist" "$CASES" "$QUINT" "$E/runs/paths/before.json"
$R paths-oracle "$W" node "$E/drivers/paths.mjs" oracle "$S/oracle-r2" "$E/oracle/dist" "$W/dist" "$CASES" "$QUINT" "$E/runs/paths/oracle.json"
$R paths-after "$W" node "$E/drivers/paths.mjs" after "$S/after" "$W/dist" "$W/dist" "$CASES" "$QUINT" "$E/runs/paths/after.json"
$R analyze-paths "$W" python3 -I "$E/drivers/analyze-paths.py" "$E/runs/paths" fixtures/nesting-chains/cases.json fixtures/nesting-chains/cases-0075.json fixtures/call-depth/cases-0119.json

export TLC_JAVA=$HOME/.local/share/mise/installs/java/temurin-21.0.11+10.0.LTS/bin/java
export TLC_JAR=$E/tools/tla2tools.jar
export TLC_APALACHE=$E/tools/apalache-0.63.0.jar
$R tlc-0119 "$W" node "$E/drivers/tlc.mjs" "$E/runs/tlc/tlc" fixtures/call-depth/cases-0119.json - '^(?!calls-)'
$R analyze-tlc "$W" python3 -I "$E/drivers/analyze-tlc.py" "$E/runs/tlc/tlc" evals/runs/0074-tlc/verdicts.json evals/runs/0075-tlc/verdicts.json "$E/runs/paths" fixtures/call-depth/cases-0119.json "$E/runs/tlc"
echo done > "$E/logs/slot-b.done"
