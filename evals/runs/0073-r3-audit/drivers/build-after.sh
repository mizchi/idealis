#!/bin/bash
E=<scratch>/eval-0073-r3
T=<after>
$E/drivers/run.sh build-after-core $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml || exit 1
$E/drivers/run.sh build-after-wasm $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml || exit 1
$E/drivers/run.sh build-after-bench $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo bench --no-run --locked --manifest-path crates/rtlc/Cargo.toml --bench safety || exit 1
echo done > $E/logs/build-after.done
