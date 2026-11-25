#!/bin/bash
# usage: E=<scratch> slot-e.sh <worktree>
# The checks listed by the plan before finishing.
: "${E:?}"
W=$1
cd "$W" || exit 1
R=$E/drivers/run.sh
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR -u CLAUDECODE NPM_CONFIG_USERCONFIG=/dev/null)
for recipe in check wasm-test safety-test quint-test portable-build portable-test; do
  $R "final-$recipe" "$W" "${U[@]}" just "$recipe"
done
$R final-go-test "$W" "${U[@]}" GOFLAGS=-buildvcs=false mise exec go@1.25.14 -- just go-test
$R final-browser "$W" "${U[@]}" just browser
$R final-portable-browser "$W" "${U[@]}" just portable-browser
$R final-mutation "$W" "${U[@]}" just mutation
$R final-support-check "$W" "${U[@]}" just support-check
echo done > "$E/logs/slot-e.done"
