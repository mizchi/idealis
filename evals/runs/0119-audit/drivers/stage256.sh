#!/bin/bash
: "${E:?}"
cd "$1" || exit 1
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR)
$E/drivers/run.sh release3-rtlc-stack-tests "$PWD" "${U[@]}" cargo test --release --locked --manifest-path crates/rtlc/Cargo.toml --test evaluation_stack --test parallel_stack
$E/drivers/run.sh release3-compiler-call_depth "$PWD" "${U[@]}" cargo test --release --locked --manifest-path crates/rtla-compiler/Cargo.toml --test call_depth
$E/drivers/run.sh release3-rtlc-all "$PWD" "${U[@]}" cargo test --release --locked --manifest-path crates/rtlc/Cargo.toml
$E/drivers/run.sh release3-compiler-all "$PWD" "${U[@]}" cargo test --release --locked --manifest-path crates/rtla-compiler/Cargo.toml
SUFFIX=-r7 $E/drivers/build-harness.sh after "$PWD"
cells=$(cat $E/scratch/stack-cells-256.txt)
for side in oracle after before; do for profile in release debug; do
  $E/drivers/run.sh stack256-$side-$profile "$PWD" node dist/evals/experiments/call-depth/stack.js $E/stack/target-$side/$profile/call-depth-stack-harness $E/runs/stack/256-$side-$profile.json "$cells"
done; done
echo done > $E/logs/stage256.done
