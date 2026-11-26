#!/bin/bash
: "${E:?}"
W=$1
cd "$W" || exit 1
R=$E/drivers/run.sh
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR NPM_CONFIG_USERCONFIG=/dev/null)
$R cli-build-after "$W" "${U[@]}" cargo build --release --locked --target wasm32-wasip1 --bin idealis --manifest-path crates/idealis-cli/Cargo.toml
$R cli-build-oracle "$E/oracle" "${U[@]}" cargo build --release --locked --target wasm32-wasip1 --bin idealis --manifest-path crates/idealis-cli/Cargo.toml
mkdir -p "$E/runs/cli" "$E/scratch/cli"
$R cli-after "$W" node dist/evals/experiments/call-depth/cli-ceiling.js "$W/crates/idealis-cli/target/wasm32-wasip1/release/idealis.wasm" "$E/scratch/cli/after" "$E/runs/cli/after.json" hop:1:300 operator:1:300
$R cli-oracle "$W" node dist/evals/experiments/call-depth/cli-ceiling.js "$E/oracle/crates/idealis-cli/target/wasm32-wasip1/release/idealis.wasm" "$E/scratch/cli/oracle" "$E/runs/cli/oracle.json" hop:1:1400 operator:1:2500
echo done > "$E/logs/slot-f.done"
