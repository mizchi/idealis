#!/bin/bash
# usage: E=<scratch> build-side.sh <side> <tree> [bench]
: "${E:?}"
side=$1; T=$2; bench=$3
U="env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR NPM_CONFIG_USERCONFIG=/dev/null"
$E/drivers/run.sh pnpm-install-$side $T env NPM_CONFIG_USERCONFIG=/dev/null pnpm install --frozen-lockfile --offline || exit 1
$E/drivers/run.sh build-$side-ts $T $U just build || exit 1
$E/drivers/run.sh build-$side-core $T $U cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml || exit 1
$E/drivers/run.sh build-$side-wasm $T $U cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml || exit 1
if [ "$bench" = bench ]; then
$E/drivers/run.sh build-$side-rtlc-bench $T $U cargo bench --no-run --locked --manifest-path crates/rtlc/Cargo.toml --bench safety --message-format=json || exit 1
fi
echo done > $E/logs/build-$side.done
