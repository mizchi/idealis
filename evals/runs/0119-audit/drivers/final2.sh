#!/bin/bash
: "${E:?}"
W=$1
cd "$W" || exit 1
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR -u CLAUDECODE NPM_CONFIG_USERCONFIG=/dev/null)
$E/drivers/run.sh final-check-r2 "$W" "${U[@]}" just check
$E/drivers/run.sh final-mutation-r2 "$W" "${U[@]}" just mutation
$E/drivers/run.sh final-support-check-r2 "$W" "${U[@]}" just support-check
$E/drivers/run.sh final-call-depth-harness "$W" "${U[@]}" just call-depth-harness
echo done > $E/logs/final2.done
