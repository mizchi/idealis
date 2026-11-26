#!/bin/bash
# usage: E=<scratch> wasm-node.sh <name> <wasm> <harness> <output> <mode: default|liftoff-only|no-liftoff> <cells...>
: "${E:?}"
name=$1; wasm=$2; harness=$3; out=$4; mode=$5; shift 5
flag=(); [ "$mode" != default ] && flag=("--$mode")
"$E/drivers/run.sh" "$name" "$PWD" node "${flag[@]}" dist/evals/experiments/call-depth/wasm-ceiling.js "$wasm" "$harness" "$out" "$@"
