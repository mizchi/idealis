#!/bin/bash
E=<scratch>/eval-0073-r2
for side in before after; do
  if [ $side = before ]; then T=<scratch>/before-0073; else T=<after>; fi
  $E/drivers/run.sh build-$side-core $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml || exit 1
  $E/drivers/run.sh build-$side-wasm $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml || exit 1
  $E/drivers/run.sh build-$side-bench $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo bench --no-run --locked --manifest-path crates/rtlc/Cargo.toml --bench safety || exit 1
done
echo done > $E/logs/build-all.done
