#!/bin/bash
S="${S:?scratch}"
W="${W:?worktree}"
cd $W
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR -u CLAUDECODE NPM_CONFIG_USERCONFIG=/dev/null)
idle() {
  for i in $(seq 1 30); do
    [ -e $S/PAUSE ] && { echo "PAUSED before $1"; while [ -e $S/PAUSE ]; do sleep 30; done; }
    n=$(pgrep -fl 'run-expected[.]ts|tlc2[.]TLC|idealis[-]core|quint[-]command|apalache[.]jar' | grep -v held-recipes.sh | wc -l | tr -d " ")
    [ "$n" = 0 ] && return 0
    echo "busy before $1: $n processes, waiting"; sleep 60
  done
  echo "BUSY-GAVE-UP $1"; return 1
}
step() {
  name=$1; shift
  idle $name || exit 3
  free=$(df -g / | tail -1 | awk '{print $4}'); echo "free ${free}G before $name"
  $S/run $name $W "$@" || exit $?
}
step s4-check "${U[@]}" just check
step s4-wasm-test "${U[@]}" just wasm-test
step s4-safety-test "${U[@]}" just safety-test
step s4-portable-test "${U[@]}" just portable-test
step s4-go-test "${U[@]}" GOFLAGS=-buildvcs=false mise exec go@1.25.14 -- just go-test
step s4-quint-test "${U[@]}" just quint-test
step s4-evaluate "${U[@]}" just evaluate evals/runs/0120-evaluate
step s4-replay-evaluation "${U[@]}" just replay-evaluation evals/runs/0120-evaluate/witnesses.json
step s4-eval-loop "${U[@]}" just eval-loop evals/plans/0002-mixed-depth.json 0120-loop
step s4-replay-loop "${U[@]}" just replay-loop evals/runs/0120-loop-confirmation
step s4-runtime-build "${U[@]}" just runtime-build
step s4-runtime-test "${U[@]}" just runtime-test
echo ALLDONE
