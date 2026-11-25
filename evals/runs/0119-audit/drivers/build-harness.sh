#!/bin/bash
# usage: E=<scratch> build-harness.sh <side> <tree> [features]
: "${E:?}"
side=$1; T=$2; features=$3
U="env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR"
F=""; [ -n "$features" ] && F="--features $features"
H=$T/evals/experiments/call-depth/stack-harness
$E/drivers/run.sh stack-build-$side-debug$SUFFIX $H $U cargo build --locked $F --target-dir $E/stack/target-$side || exit 1
$E/drivers/run.sh stack-build-$side-release$SUFFIX $H $U cargo build --release --locked $F --target-dir $E/stack/target-$side || exit 1
