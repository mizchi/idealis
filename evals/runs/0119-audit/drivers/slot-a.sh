#!/bin/bash
# usage: E=<scratch> slot-a.sh <worktree>
# Rebuilds the oracle from its final source, freezes snapshots, regenerates the baselines and fixtures, builds after.
: "${E:?}"
W=$1
cd "$W" || exit 1
U=(env -u CARGO_TARGET_DIR -u CARGO_BUILD_TARGET_DIR NPM_CONFIG_USERCONFIG=/dev/null)
R=$E/drivers/run.sh
S=$E/snapshots

$R oracle-r2-build-core "$E/oracle" "${U[@]}" cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml || exit 1
$R oracle-r2-build-wasm "$E/oracle" "${U[@]}" cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml || exit 1
mkdir -p "$S/oracle-r2"
cp "$E/oracle/crates/idealis-core/target/release/idealis-core" "$S/oracle-r2/"
cp "$E/oracle/crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm" "$S/oracle-r2/"
(cd "$S/oracle-r2" && shasum -a 256 idealis-core idealis_core.wasm > sha256.txt)

$R baseline-before-0119-r3 "$W" node dist/evals/experiments/call-depth/baseline.js "$S/before-r2/idealis-core" "$S/before-r2/idealis_core.wasm" "$E/before/dist" "$E/runs/oracle/baseline-before-0119-r3.json" || exit 1
$R baseline-oracle-0119-r3 "$W" node dist/evals/experiments/call-depth/baseline.js "$S/oracle-r2/idealis-core" "$S/oracle-r2/idealis_core.wasm" "$E/oracle/dist" "$E/runs/oracle/baseline-oracle-0119-r3.json" || exit 1
rm -f fixtures/call-depth/records-0119.json fixtures/call-depth/cases-0119.json
$R expect-0119-r2 "$W" node dist/evals/experiments/call-depth/expect-0119.js "$E/runs/oracle/baseline-before-0119-r3.json" "$E/runs/oracle/baseline-oracle-0119-r3.json" fixtures/call-depth/records-0119.json fixtures/call-depth/cases-0119.json || exit 1

$R red2-ts-call-depth "$W" env -u CLAUDECODE node --test dist/tests/call-depth/contract-test.js
$R red2-ts-nesting-chains "$W" env -u CLAUDECODE node --test dist/tests/nesting-chains/contract-test.js
$R red2-wasm-build "$W" "${U[@]}" just wasm-build
$R red2-wasm-callDepth "$W" env -u CLAUDECODE node --test --test-name-pattern="runCallDepth" dist/tests/wasm-host/contract-test.js
cp "$W/crates/rtla-compiler/tests/nesting_chains.rs" "$E/before/crates/rtla-compiler/tests/nesting_chains.rs"
cp "$W"/fixtures/call-depth/*.json "$E/before/fixtures/call-depth/"
$R red2-compiler-nesting_chains "$E/before" "${U[@]}" cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test nesting_chains

$R build-after-safety "$W" "${U[@]}" just safety-build || exit 1
$R build-after-rtlc-bench "$W" "${U[@]}" cargo bench --no-run --locked --manifest-path crates/rtlc/Cargo.toml --bench safety --message-format=json || exit 1
src=$(grep -o '"executable":"[^"]*safety-[a-f0-9]*"' "$E/logs/build-after-rtlc-bench.log" | head -1 | sed 's/"executable":"//; s/"$//')
$R copy-after-bench-to-deps "$W" "$E/drivers/copy-bench-into-deps.sh" "$W" "$src" || exit 1
$R freeze-after "$W" env IDEALIS_COMPACT_PLAN=0119-call-depth-limit node dist/evals/experiments/compact-functions/freeze.js "$S/after" || exit 1
$E/drivers/protected.sh "$W" > "$E/scratch/protected-after-final.txt"
echo done > "$E/logs/slot-a.done"
