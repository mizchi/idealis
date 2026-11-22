#!/bin/bash
E=<scratch>/eval
for side in before after; do
  T=$E/stack/$side
  $E/drivers/run.sh stack-build-$side-debug $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo build --locked || exit 1
  $E/drivers/run.sh stack-build-$side-release $T env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR cargo build --release --locked || exit 1
done
echo done > $E/logs/build-stack.done
