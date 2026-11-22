#!/bin/bash
# usage: build-rust.sh <side> <tree>
E=<scratch>/eval
side=$1; T=$2
U="env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR"
$E/drivers/run.sh build-$side-core $T $U cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml || exit 1
$E/drivers/run.sh build-$side-wasm $T $U cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml || exit 1
$E/drivers/run.sh build-$side-rtlc-bench $T $U cargo bench --no-run --locked --manifest-path crates/rtlc/Cargo.toml --bench safety --message-format=json || exit 1
$E/drivers/run.sh build-$side-compiler-bench $T $U cargo bench --no-run --locked --manifest-path crates/rtla-compiler/Cargo.toml --message-format=json || exit 1
echo done > $E/logs/build-$side.done
