#!/bin/bash
: "${E:?}"
cd "$1" || exit 1
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR NPM_CONFIG_USERCONFIG=/dev/null)
$E/drivers/run.sh just-evaluate "$PWD" "${U[@]}" just evaluate $E/runs/evaluate
$E/drivers/run.sh just-replay-evaluation "$PWD" "${U[@]}" just replay-evaluation $E/runs/evaluate/witnesses.json
$E/drivers/run.sh just-eval-loop "$PWD" "${U[@]}" just eval-loop evals/plans/0002-mixed-depth.json 0119-loop
$E/drivers/run.sh just-replay-loop "$PWD" "${U[@]}" just replay-loop evals/runs/0119-loop-confirmation
echo done > $E/logs/loops.done
