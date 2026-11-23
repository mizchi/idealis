#!/bin/bash
# usage: build-rust.sh <side> <tree>
E=<scratch>
side=$1; T=$2
U="env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR NPM_CONFIG_USERCONFIG=/dev/null"
$E/drivers/run.sh build-$side-ts $T $U just build || exit 1
$E/drivers/run.sh build-$side-core $T $U cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml || exit 1
$E/drivers/run.sh build-$side-wasm $T $U cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml || exit 1
echo done > $E/logs/build-$side.done
