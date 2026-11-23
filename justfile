default:
  @just --list

support-check:
  node --experimental-strip-types scripts/check-support.ts

support-check-staged:
  node --experimental-strip-types scripts/check-support.ts --staged

[positional-arguments]
support-review *args:
  node --experimental-strip-types scripts/check-support.ts --replay "$@"

support-plan:
  node --experimental-strip-types scripts/check-support.ts --dry-run

support-sync:
  node --experimental-strip-types scripts/check-support.ts --sync

support-sync-staged:
  node --experimental-strip-types scripts/check-support.ts --sync --staged

support-compact:
  node --experimental-strip-types scripts/check-support.ts --compact

support-hooks:
  node --experimental-strip-types scripts/check-support.ts --install-hook

support-test: build
  node --test dist/scripts/check-support-test.js

support-eval run: build
  node dist/evals/experiments/crate-checklists/run.js {{quote(run)}}

support-replay run: build
  node dist/evals/experiments/crate-checklists/run.js {{quote(run)}} --replay

[positional-arguments]
connect-bindings model module template *options:
  @node scripts/connect-bindings.ts "$@"

connect-conformance-test: build
  node --test dist/tests/connect/conformance.test.js

connect-bindings-test:
  node --test tests/connect/bindings.test.ts

oracle-types:
  node --experimental-strip-types scripts/oracle-types.ts

oracle-types-check:
  node --experimental-strip-types scripts/oracle-types.ts --check

oracle-native-build: build
  node dist/scripts/build-native-oracle.js --check

oracle-native-test: oracle-native-build oracle-wire-build
  cargo fmt --manifest-path evals/experiments/roracle-native/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/roracle-native/rust/Cargo.toml -- -D warnings
  node --test dist/tests/roracle-native/contract-test.js
  pnpm exec vp test run --project=native-oracle
  IDEALIS_ORACLE_BACKEND=native-json node --test dist/tests/workspace-oracle/contract-test.js
  IDEALIS_ORACLE_BACKEND=native-object node --test dist/tests/workspace-oracle/contract-test.js

oracle-native-eval output: build
  node dist/evals/experiments/roracle-native/run.js {{quote(output)}}

oracle-native-replay run output: build
  node dist/evals/experiments/roracle-native/replay.js {{quote(run)}} {{quote(output)}}

oracle-object-phases output mode="measure" phase="object-request" fixture="nested-256": build
  node dist/evals/experiments/roracle-native/phases.js {{quote(output)}} {{quote(mode)}} {{quote(phase)}} {{quote(fixture)}}

oracle-object-compare before output control="": build
  node dist/evals/experiments/roracle-native/compare-object.js {{quote(before)}} {{quote(output)}} {{quote(control)}}

oracle-object-replay before saved output: build
  node dist/evals/experiments/roracle-native/replay-object.js {{quote(before)}} {{quote(saved)}} {{quote(output)}}

oracle-typed-build:
  node --experimental-strip-types evals/experiments/roracle-wit/build.ts --check

oracle-typed-test: oracle-typed-build oracle-wire-build oracle-native-build build
  cargo fmt --manifest-path evals/experiments/roracle-wit/rust/Cargo.toml -- --check
  cargo clippy --all-targets --all-features --locked --manifest-path evals/experiments/roracle-wit/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/experiments/roracle-wit/rust/Cargo.toml
  node --test dist/tests/roracle-wit/contract-test.js
  pnpm exec vp test run --project=typed-oracle

oracle-typed-browser: oracle-typed-build oracle-wire-build
  pnpm exec playwright test --project=typed-oracle

oracle-typed-eval output: build
  node dist/evals/experiments/roracle-wit/measure.js {{quote(output)}}

oracle-typed-replay saved output: build
  node dist/evals/experiments/roracle-wit/replay.js {{quote(saved)}} {{quote(output)}}

oracle-typed-diagnose saved output: build
  node dist/evals/experiments/roracle-wit/diagnose.js {{quote(saved)}} {{quote(output)}}

oracle-wire-build:
  cargo build --release --locked --manifest-path crates/roracle/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/roracle/Cargo.toml

workspace-oracle-build: oracle-wire-build
  cargo build --release --locked --manifest-path evals/experiments/workspace-oracle/rust/Cargo.toml

workspace-oracle-test: workspace-oracle-build portable-build testing-build ralloy-build rusmt-build rtla-build build
  cargo fmt --manifest-path evals/experiments/workspace-oracle/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/workspace-oracle/rust/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path evals/experiments/workspace-oracle/rust/Cargo.toml
  node --test dist/tests/workspace-oracle/contract-test.js dist/tests/oracle-tuning/wasm-test.js dist/tests/workspace-oracle/catalog-test.js

# Run builds and tests first; snapshots freeze their executables and sources.
workspace-snapshot directory: build
  node dist/evals/experiments/workspace-oracle/tune.js snapshot {{quote(directory)}}

workspace-bench baseline candidate run:
  node dist/evals/experiments/workspace-oracle/tune.js run {{quote(baseline)}} {{quote(candidate)}} {{quote(run)}}

workspace-replay run replay:
  node dist/evals/experiments/workspace-oracle/tune.js replay {{quote(run)}} {{quote(replay)}}

workspace-oracle-cost run:
  node dist/evals/experiments/workspace-oracle/cost.js {{quote(run)}}

workspace-oracle-cost-replay run replay:
  node dist/evals/experiments/workspace-oracle/cost.js {{quote(replay)}} {{quote(run)}}

# Build and test first. Frozen binaries keep compilation outside the measurement.
oracle-tuning-freeze directory:
  node dist/evals/experiments/oracle-tuning/run.js freeze {{quote(directory)}}

oracle-tuning-measure before transport after run:
  node dist/evals/experiments/oracle-tuning/run.js measure {{quote(before)}} {{quote(transport)}} {{quote(after)}} {{quote(run)}}

oracle-tuning-compare before wire combined run:
  node dist/evals/experiments/oracle-tuning/run.js compare {{quote(before)}} {{quote(wire)}} {{quote(combined)}} {{quote(run)}}

oracle-tuning-phases snapshot run mode="measure":
  node dist/evals/experiments/oracle-tuning/phases.js {{quote(snapshot)}} {{quote(run)}} {{quote(mode)}}

oracle-tuning-compat before after run:
  node dist/evals/experiments/oracle-tuning/compat.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

oracle-tuning-kernel before after run:
  node dist/evals/experiments/oracle-tuning/kernel.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

oracle-criterion:
  cargo bench --locked --manifest-path crates/roracle/Cargo.toml --bench wire

oracle-build: oracle-wire-build
  cargo build --release --locked --manifest-path evals/experiments/oracle-comparison/rust/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path evals/experiments/oracle-comparison/rust/Cargo.toml

oracle-test: oracle-build workspace-oracle-test
  cargo fmt --manifest-path crates/roracle/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/roracle/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path crates/roracle/Cargo.toml
  cargo test --locked --manifest-path crates/roracle/Cargo.toml --bench wire
  cargo fmt --manifest-path evals/experiments/oracle-comparison/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/oracle-comparison/rust/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path evals/experiments/oracle-comparison/rust/Cargo.toml
  cargo run --example comparison --locked --manifest-path crates/roracle/Cargo.toml
  node --test --test-name-pattern="roracle|typed oracle" dist/tests/wasm-host/contract-test.js

# Build and run other checks before this recipe; measurements run alone.
oracle-eval run java jar: build
  node dist/evals/experiments/oracle-comparison/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

oracle-replay run: build
  node dist/evals/experiments/oracle-comparison/replay.js {{quote(run)}}

fuzz-build: testing-build
  cargo build --release --locked --manifest-path evals/experiments/structured-fuzz/rust/Cargo.toml
  cargo +nightly-2026-08-27 metadata --locked --format-version 1 --manifest-path evals/experiments/structured-fuzz/fuzz/Cargo.toml > /dev/null
  cargo +nightly-2026-08-27 fuzz build --sanitizer address --fuzz-dir evals/experiments/structured-fuzz/fuzz

fuzz-test: fuzz-build build
  cargo fmt --manifest-path evals/experiments/structured-fuzz/rust/Cargo.toml -- --check
  cargo fmt --manifest-path evals/experiments/structured-fuzz/fuzz/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/structured-fuzz/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/experiments/structured-fuzz/rust/Cargo.toml
  node --test dist/tests/structured-fuzz/contract-test.js
  pnpm exec vp test run --project=fuzz

fuzz-browser:
  pnpm exec playwright test --project=fuzz

fuzz-eval phase run parent="": fuzz-build build
  node dist/evals/experiments/structured-fuzz/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

fuzz-replay run: build
  node dist/evals/experiments/structured-fuzz/replay.js {{quote(run)}}

controlled-build: testing-build
  cargo build --release --locked --manifest-path evals/experiments/controlled-schedules/rust/Cargo.toml
  cargo build --release --locked --example loom --manifest-path evals/experiments/controlled-schedules/rust/Cargo.toml

controlled-test: controlled-build testing-core-test build
  cargo fmt --manifest-path evals/experiments/controlled-schedules/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/controlled-schedules/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/experiments/controlled-schedules/rust/Cargo.toml
  cargo run --release --locked --example loom --manifest-path evals/experiments/controlled-schedules/rust/Cargo.toml
  node --test dist/tests/controlled-schedules/contract-test.js
  pnpm exec vp test run --project=schedules

controlled-browser: controlled-build
  pnpm exec playwright test --project=schedules

controlled-eval phase run parent="": controlled-build build
  node dist/evals/experiments/controlled-schedules/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

controlled-replay run: build
  node dist/evals/experiments/controlled-schedules/replay.js {{quote(run)}}

sat-build:
  cargo build --release --locked --manifest-path evals/experiments/sat-foundation/rust/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path evals/experiments/sat-foundation/rust/Cargo.toml

sat-test: sat-build build
  cargo fmt --manifest-path evals/experiments/sat-foundation/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/sat-foundation/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/experiments/sat-foundation/rust/Cargo.toml
  node --test dist/tests/sat-foundation/contract-test.js

sat-browser: sat-build
  pnpm exec playwright test --project=sat

sat-eval phase run parent="": sat-build build
  node dist/evals/experiments/sat-foundation/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

sat-replay run: build
  node dist/evals/experiments/sat-foundation/replay.js {{quote(run)}}

testing-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/idealis-cli/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-connect/Cargo.toml

testing-core-test:
  cargo fmt --manifest-path crates/idealis-connect/Cargo.toml -- --check
  cargo fmt --manifest-path crates/idealis-cli/Cargo.toml -- --check
  cargo clippy --all-targets --all-features --locked --manifest-path crates/idealis-connect/Cargo.toml -- -D warnings
  cargo clippy --all-targets --locked --manifest-path crates/idealis-cli/Cargo.toml -- -D warnings
  cargo test --all-targets --all-features --locked --manifest-path crates/idealis-connect/Cargo.toml
  cargo test --all-targets --locked --manifest-path crates/idealis-cli/Cargo.toml

testing-example:
  cargo run --example counter --locked --manifest-path crates/idealis-connect/Cargo.toml

testing-test: testing-build build
  node --test dist/tests/testing-core/contract-test.js
  pnpm exec vp test run --project=testing

testing-browser: testing-build
  pnpm exec playwright test --project=testing

testing-eval phase run parent="": testing-build build
  node dist/evals/experiments/testing-core/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

testing-replay run: build
  node dist/evals/experiments/testing-core/replay.js {{quote(run)}}

ralloy-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/ralloy/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/ralloy/Cargo.toml

ralloy-test:
  cargo fmt --manifest-path crates/ralloy/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/ralloy/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/ralloy/Cargo.toml

# Records Alloy 6.2.0's answers for tests/alloy/<group> (plan 0078).
ralloy-oracle-record group java=".artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java" jar=".artifacts/alloy-subset/org.alloytools.alloy.dist.jar":
  cargo run --release --locked --manifest-path crates/ralloy/Cargo.toml --example record_alloy -- {{quote(group)}} {{quote(absolute_path(java))}} {{quote(absolute_path(jar))}}

ralloy-oracle-bench:
  cargo bench --locked --manifest-path crates/ralloy/Cargo.toml --bench oracle

ralloy-example:
  cargo run --example graph --locked --manifest-path crates/ralloy/Cargo.toml

ralloy-contract: ralloy-build build
  node --test dist/tests/alloy-subset/contract-test.js

ralloy-browser: ralloy-build
  pnpm exec playwright test --project=alloy

ralloy-eval phase run java jar parent="": ralloy-build build
  node dist/evals/experiments/alloy-subset/run.js {{quote(phase)}} {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(parent)}}

ralloy-replay run: build
  node dist/evals/experiments/alloy-subset/replay.js {{quote(run)}}

alloy-test: ralloy-build build
  node --test dist/tests/alloy-instance/contract-test.js
  pnpm exec vp test run --project=alloy

alloy-browser: ralloy-build
  pnpm exec playwright test --project=alloy-instance

alloy-instance-eval phase run java jar parent="": ralloy-build build
  node dist/evals/experiments/alloy-instance/run.js {{quote(phase)}} {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(parent)}}

alloy-instance-replay run: build
  node dist/evals/experiments/alloy-instance/replay.js {{quote(run)}}

rusmt-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/rusmt/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/rusmt/Cargo.toml

rusmt-test:
  cargo fmt --manifest-path crates/rusmt/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/rusmt/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/rusmt/Cargo.toml

rusmt-example:
  cargo run --example ttl --locked --manifest-path crates/rusmt/Cargo.toml

rusmt-compare run baseline: rusmt-build build
  node dist/evals/experiments/rsmt-extraction/compare.js check {{quote(run)}} {{quote(baseline)}}

rusmt-criterion:
  cargo bench --locked --manifest-path crates/rusmt/Cargo.toml --bench solver

alias rsmt-build := rusmt-build
alias rsmt-test := rusmt-test
alias rsmt-example := rusmt-example
alias rsmt-compare := rusmt-compare
alias rsmt-criterion := rusmt-criterion

smt-build: rusmt-build

smt-test: rusmt-test

smt-criterion: rusmt-criterion

smt-verify phase run parent="": smt-build build
  node dist/evals/experiments/smt-subset/verify-run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

smt-bench run z3: smt-build build
  node dist/evals/experiments/smt-subset/benchmark.js {{quote(run)}} {{quote(z3)}}

smt-browser: smt-build
  pnpm exec playwright test --project=smt

install:
  pnpm install --frozen-lockfile

build-sdk:
  pnpm exec tsc -p packages/idealis/tsconfig.json

build: build-sdk
  pnpm exec tsc -p tsconfig.json

typecheck:
  pnpm exec tsc --noEmit

lint:
  pnpm exec vp lint

fmt:
  pnpm exec vp fmt --write

check: style-check typecheck test

style-check:
  pnpm exec vp check

test: (vitest "--project=core") test-node

[positional-arguments]
vitest *args:
  pnpm exec vp test run "$@"

test-node: build
  node --test dist/tests/node.test.js

mutation:
  pnpm exec mutator

wasm-build: build
  node dist/scripts/build-wasm-tests.js

wasm-test: wasm-build
  node --test dist/tests/wasm-host/contract-test.js
  pnpm exec vp test run --project=wasm

# Optional compatibility checks for browser workers and isolation.
browser: portable-build
  pnpm exec playwright test --project=core

demo: build
  node dist/examples/demo.js

[positional-arguments]
evaluate *args: build
  node dist/evals/experiments/harness/evaluate.js "$@"

[positional-arguments]
replay-evaluation *args: build
  node dist/evals/experiments/harness/replay.js "$@"

[positional-arguments]
eval-loop plan="evals/plans/0002-mixed-depth.json" *args: build
  node dist/evals/experiments/loop.js "$@"

[positional-arguments]
replay-loop *args: build
  node dist/evals/experiments/replay.js "$@"

runtime-build:
  cargo build --release --manifest-path evals/experiments/rust-probe/Cargo.toml
  cargo build --release --lib --target wasm32-unknown-unknown --manifest-path evals/experiments/rust-probe/Cargo.toml

runtime-test: runtime-build build
  cargo test --manifest-path evals/experiments/rust-probe/Cargo.toml
  node --test dist/tests/runtime/rust-test.js dist/tests/runtime/system-test.js
  node --test --test-name-pattern="runtime Wasm" dist/tests/wasm-host/contract-test.js

runtime-browser: runtime-build build
  pnpm exec playwright test --project=runtime

[positional-arguments]
eval-shard +args: build
  node dist/evals/experiments/runtime/shard-cli.js "$@"

[positional-arguments]
runtime-eval *args: build
  node dist/evals/experiments/runtime/evaluate.js "$@"

portable-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/idealis-core/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-core/Cargo.toml

portable-test: portable-build rtlc-test rtla-test compiler-test
  cargo test --all-targets --locked --manifest-path crates/idealis-core/Cargo.toml
  node --test dist/tests/portable/contract-test.js dist/tests/action-coverage/contract-test.js
  node --test --test-name-pattern="portable Wasm|Quint generated|parser Wasm" dist/tests/wasm-host/contract-test.js

portable-browser: portable-build rtla-build build
  pnpm exec playwright test --project portable rtla

[positional-arguments]
portable-eval *args: build
  node dist/evals/experiments/portable/evaluate.js "$@"

portable-replay run: build
  node dist/evals/experiments/portable/replay.js {{quote(run)}}

safety-build: portable-build build

safety-test: safety-build
  node --test dist/tests/rust-safety/contract-test.js dist/tests/collections-temporal/contract-test.js dist/tests/tlc-differential/compare-test.js dist/tests/tlc-differential/regression-test.js dist/tests/nesting-chains/contract-test.js

safety-eval run plan="0014-value-hash": safety-build
  node dist/evals/experiments/rust-safety/evaluate.js {{quote(run)}} {{quote(plan)}}

safety-replay run: build
  node dist/evals/experiments/rust-safety/replay.js {{quote(run)}}

safety-tlc java jar: safety-build
  node dist/evals/experiments/rust-safety/tlc-check.js {{quote(java)}} {{quote(jar)}}

go-test: portable-build build
  node dist/evals/experiments/go/prepare.js
  test -z "$(gofmt -l adapters/go)"
  cd adapters/go && go vet ./...
  cd adapters/go && CGO_ENABLED=0 go test -count=1 ./...
  cd adapters/go && go test -race -count=1 ./...
  node dist/scripts/go-consumer.js

cross-build: portable-build build
  cargo build --release --locked --manifest-path evals/experiments/shared-oracle-rust/Cargo.toml
  mkdir -p .artifacts/go
  cd adapters/go && CGO_ENABLED=0 go build -o ../../.artifacts/go/counter-replay ./cmd/counter-replay

cross-test: cross-build
  cargo fmt --manifest-path evals/experiments/shared-oracle-rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/experiments/shared-oracle-rust/Cargo.toml -- -D warnings
  node --test dist/tests/cross-language/contract-test.js

cross-eval phase run parent="": cross-build
  node dist/evals/experiments/cross-language/evaluate.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

cross-replay run: build
  node dist/evals/experiments/cross-language/replay.js {{quote(run)}}

async-build: cross-build
  cd adapters/go && CGO_ENABLED=0 go build -o ../../.artifacts/go/history-check ./cmd/history-check
  cd adapters/go && CGO_ENABLED=0 go build -o ../../.artifacts/go/finite-bench ./cmd/finite-bench

async-test: async-build
  node --test dist/tests/async-tuning/contract-test.js

async-eval phase run parent="": async-build
  node dist/evals/experiments/async-tuning/evaluate.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

async-replay run: build
  node dist/evals/experiments/async-tuning/replay.js {{quote(run)}}

usable-test: build
  node --test dist/tests/usable/contract-test.js

[positional-arguments]
usable-eval *args: build
  node dist/evals/experiments/usable/evaluate.js "$@"

usable-replay run: build
  node dist/evals/experiments/usable/replay.js {{quote(run)}}

usable-benchmark input output: build
  node dist/evals/experiments/usable/benchmark-run.js {{quote(input)}} {{quote(output)}}

package: package-prepare package-test

[positional-arguments]
package-prepare *args: build rusmt-build
  node dist/scripts/package.js "$@"

[positional-arguments]
package-test *args: build
  node dist/scripts/package-test.js "$@"

cli-wasm-build:
  cargo build --release --locked --target wasm32-wasip1 --bin idealis --manifest-path crates/idealis-cli/Cargo.toml

cli-compare java jar: package-prepack
  node --experimental-strip-types scripts/compare-tla-cli.ts {{quote(java)}} {{quote(jar)}}

package-prepack: cli-wasm-build build-sdk testing-build ralloy-build portable-build rtla-build
  node --experimental-strip-types scripts/prepare-npm-assets.ts

complex-tlc java jar run: safety-build
  node dist/evals/experiments/collections-temporal/tlc-check.js {{quote(java)}} {{quote(jar)}} {{quote(run)}}

complex-eval run parent="": safety-build
  node dist/evals/experiments/collections-temporal/evaluate.js {{quote(run)}} {{quote(parent)}}

complex-replay run: build
  node dist/evals/experiments/collections-temporal/replay.js {{quote(run)}}

complex-tlc-replay java jar run: build
  node dist/evals/experiments/collections-temporal/tlc-replay.js {{quote(java)}} {{quote(jar)}} {{quote(run)}}


tlc-modules: build
  node dist/scripts/prepare-tlc-modules.js

tlc-diff-test java jar: safety-build tlc-modules
  node --test dist/tests/tlc-differential/compare-test.js dist/tests/tlc-differential/standard-modules-test.js
  TLC_JAVA={{quote(java)}} TLC_JAR={{quote(jar)}} node --test dist/tests/tlc-differential/standard-modules-tlc-test.js


tlc-diff java jar phase run: safety-build tlc-modules
  node dist/evals/experiments/tlc-differential/run.js {{quote(java)}} {{quote(jar)}} {{quote(phase)}} {{quote(run)}}

tlc-diff-replay java jar saved run: safety-build tlc-modules
  node dist/evals/experiments/tlc-differential/run.js {{quote(java)}} {{quote(jar)}} replay {{quote(run)}} {{quote(saved)}}

checker-bench-test: build
  node --test dist/tests/checker-benchmark/contract-test.js

checker-bench java jar archive run: safety-build
  node dist/evals/experiments/checker-benchmark/run.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

checker-bench-replay java jar archive saved run: safety-build
  node dist/evals/experiments/checker-benchmark/replay.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(saved)}} {{quote(run)}}

parallel-safety-test: safety-build
  node --test dist/tests/parallel-safety/contract-test.js

parallel-safety-eval run: safety-build
  node dist/evals/experiments/parallel-safety/evaluate.js {{quote(run)}}

parallel-safety-replay saved run: safety-build
  node dist/evals/experiments/parallel-safety/replay.js {{quote(saved)}} {{quote(run)}}

tlc-throughput java jar archive run: safety-build
  node dist/evals/experiments/tlc-throughput/run.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

tlc-throughput-replay java jar archive saved run: safety-build
  node dist/evals/experiments/tlc-throughput/run.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

single-model-test java jar: safety-build
  IDEALIS_TLC_JAVA={{quote(java)}} IDEALIS_TLC_JAR={{quote(jar)}} node --test dist/tests/single-model/contract-test.js dist/tests/single-model/integration-test.js

single-model-eval java jar archive run: safety-build
  node dist/evals/experiments/single-model/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

single-model-replay java jar archive saved run: safety-build
  node dist/evals/experiments/single-model/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

frontier-safety-test: safety-build
  cargo test --all-targets --locked --manifest-path crates/idealis-core/Cargo.toml
  node --test dist/tests/frontier-safety/contract-test.js

frontier-safety-eval java jar archive run: safety-build
  node dist/evals/experiments/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

frontier-safety-replay java jar archive saved run: safety-build
  node dist/evals/experiments/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

frontier-budget-eval java jar archive run: safety-build
  IDEALIS_FRONTIER_PLAN=0021-worker-budget node dist/evals/experiments/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

frontier-budget-replay java jar archive saved run: safety-build
  IDEALIS_FRONTIER_PLAN=0021-worker-budget node dist/evals/experiments/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

readme-bench-test java jar: safety-build
  IDEALIS_TLC_JAVA={{quote(java)}} IDEALIS_TLC_JAR={{quote(jar)}} node --test dist/tests/readme-comparison/contract-test.js dist/tests/readme-comparison/integration-test.js

readme-bench java jar archive run: safety-build
  node dist/evals/experiments/readme-comparison/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

readme-bench-replay java jar archive saved run: safety-build
  node dist/evals/experiments/readme-comparison/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

criterion-prepare: build
  node dist/evals/experiments/criterion-safety/prepare.js

criterion-test: criterion-prepare
  cargo test --locked --manifest-path crates/rtlc/Cargo.toml --bench safety
  node --test dist/tests/criterion-safety/contract-test.js

criterion-save stage run: criterion-prepare
  node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

criterion-controls run: safety-build
  node dist/evals/experiments/criterion-safety/compare.js {{quote(run)}} controls

criterion-compare run: safety-build
  node dist/evals/experiments/criterion-safety/compare.js {{quote(run)}}

criterion-replay saved run: build
  node dist/evals/experiments/criterion-safety/compare.js {{quote(run)}} {{quote(saved)}}

profile-criterion stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0024-profile-tuning node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

profile-record backend binary run: build
  node dist/evals/experiments/profile-tuning/record.js {{quote(backend)}} {{quote(binary)}} {{quote(run)}}

profile-test: criterion-prepare
  node --test dist/tests/profile-tuning/hotspots-test.js dist/tests/profile-tuning/wasm-names-test.js

profile-summary saved run: build
  node dist/evals/experiments/profile-tuning/summarize.js {{quote(saved)}} {{quote(run)}}

profile-scalar run: safety-build
  node dist/evals/experiments/profile-tuning/scalar-compare.js {{quote(run)}}

profile-replay saved run: build
  node dist/evals/experiments/profile-tuning/scalar-compare.js {{quote(run)}} {{quote(saved)}}

rtlc-test:
  cargo fmt --manifest-path crates/rtlc/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/rtlc/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/rtlc/Cargo.toml

rtlc-example:
  cargo run --example counter --locked --manifest-path crates/rtlc/Cargo.toml

compact-functions-freeze directory:
  node dist/evals/experiments/compact-functions/freeze.js {{quote(directory)}}

compact-functions-compare before after java jar archive run:
  node dist/evals/experiments/compact-functions/compare.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

compact-functions-kernel before after run:
  node dist/evals/experiments/oracle-tuning/kernel.js {{quote(before)}} {{quote(after)}} {{quote(run)}} safety-bench

compact-functions-summary comparison criterion run:
  node dist/evals/experiments/compact-functions/summarize.js {{quote(comparison)}} {{quote(criterion)}} {{quote(run)}}

rtlc-compare run: build
  node dist/evals/experiments/rtlc-extraction/compare.js {{quote(run)}}

rtla-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/rtla/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/rtla/Cargo.toml

compiler-test:
  cargo fmt --manifest-path crates/rtla-compiler/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/rtla-compiler/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/rtla-compiler/Cargo.toml

recursive-actions-test: portable-build build
  cargo test --locked --manifest-path crates/rtlc/Cargo.toml --test recursive_actions
  cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test recursive_actions
  node --test dist/tests/recursive-actions/contract-test.js

recursive-actions-oracle run java jar: safety-build
  node dist/evals/experiments/recursive-actions/execute.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

recursive-actions-freeze directory: build
  node dist/evals/experiments/recursive-actions/bench.js freeze {{quote(directory)}}

recursive-actions-bench snapshot run: build
  node dist/evals/experiments/recursive-actions/bench.js run {{quote(run)}} {{quote(snapshot)}}

recursive-actions-profile snapshot run: build
  node dist/evals/experiments/recursive-actions/profile.js {{quote(run)}} {{quote(snapshot)}}

frontend-semantics-test: portable-build build
  cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test levels --test lazy_functions --test templates
  cargo test --locked --manifest-path crates/rtlc/Cargo.toml --test temporal_calls
  node --test dist/tests/frontend-semantics/contract-test.js

frontend-semantics-runtime directory: build
  node dist/evals/experiments/frontend-semantics/runtime.js {{quote(directory)}}

frontend-semantics-oracle snapshot run java jar: build
  IDEALIS_FRONTEND_SNAPSHOT={{quote(snapshot)}} node dist/evals/experiments/frontend-semantics/execute.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

frontend-semantics-freeze directory: build
  node dist/evals/experiments/frontend-semantics/bench.js freeze {{quote(directory)}}

frontend-semantics-bench snapshot run: build
  node dist/evals/experiments/frontend-semantics/bench.js run {{quote(run)}} {{quote(snapshot)}}

frontend-semantics-profile snapshot run: build
  node dist/evals/experiments/frontend-semantics/profile.js {{quote(run)}} {{quote(snapshot)}}

coverage-test: safety-build
  node --test dist/tests/action-coverage/contract-test.js

coverage-eval run java jar: safety-build
  node dist/evals/experiments/action-coverage/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

coverage-replay saved run java jar: safety-build
  node dist/evals/experiments/action-coverage/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

coverage-controls stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0050-action-coverage node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

coverage-bench stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0050-action-coverage IDEALIS_CRITERION_TARGET=rtlc-coverage node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

module-eval run java jar: safety-build
  node dist/evals/experiments/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

module-replay saved run java jar: safety-build
  node dist/evals/experiments/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

operators-eval run java jar: safety-build
  IDEALIS_TLC_SUITE=fairness-operators node dist/evals/experiments/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

operators-replay saved run java jar: safety-build
  IDEALIS_TLC_SUITE=fairness-operators node dist/evals/experiments/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

tlc-finite-eval run java jar: safety-build
  node dist/evals/experiments/tlc-finite/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

tlc-finite-replay saved run java jar: safety-build
  node dist/evals/experiments/tlc-finite/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

compiler-criterion stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0048-fairness-operators IDEALIS_CRITERION_TARGET=rtla-compiler node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

temporal-profile run: build
  cargo build --release --locked --manifest-path crates/rtla-compiler/Cargo.toml --example temporal-profile
  node dist/evals/experiments/temporal-tuning/profile.js crates/rtla-compiler/target/release/examples/temporal-profile {{quote(run)}}

temporal-criterion stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0049-temporal-tuning IDEALIS_CRITERION_TARGET=rtla-compiler node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

temporal-controls stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0049-temporal-tuning node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

temporal-parity baseline run: safety-build
  node dist/evals/experiments/temporal-tuning/parity.js {{quote(baseline)}} {{quote(run)}}

temporal-compare before after run: build
  node dist/evals/experiments/temporal-tuning/compare.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

temporal-alternating run: build
  node dist/evals/experiments/temporal-tuning/alternating.js {{quote(run)}}

criterion-baseline revision stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0046-tlc-finite-support IDEALIS_CRITERION_REVISION={{quote(revision)}} node dist/evals/experiments/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

criterion-verify run: build
  node dist/evals/experiments/criterion-safety/verify.js {{quote(run)}}

compiler-eval run: portable-build build
  node dist/evals/experiments/rtla-compiler/evaluate.js {{quote(run)}}

compiler-tlc run java jar: portable-build build
  node dist/evals/experiments/rtla-compiler/tlc.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

rtla-test: rtla-build build
  cargo fmt --manifest-path crates/rtla/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/rtla/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/rtla/Cargo.toml
  node --test dist/tests/rtla-parser/contract-test.js

rtla-browser: rtla-build
  pnpm exec playwright test --project=rtla

rtla-eval run java jar: rtla-build build
  node dist/evals/experiments/rtla-parser/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

rtla-compat run upstream java_home jar snapshot version: build
  node dist/evals/experiments/rtla-compatibility/run.js {{quote(run)}} {{quote(upstream)}} {{quote(java_home)}} {{quote(jar)}} {{quote(snapshot)}} {{quote(version)}}

rtla-compat-test: rtla-build build
  node --test dist/tests/rtla-compatibility/contract-test.js

rtla-symbols run java_home stable_jar preview_jar snapshot: build
  node dist/evals/experiments/symbolic-operators/run.js {{quote(run)}} {{quote(java_home)}} {{quote(stable_jar)}} {{quote(preview_jar)}} {{quote(snapshot)}}

rtla-symbols-tlc run java jar: portable-build build
  node dist/evals/experiments/symbolic-operators/tlc.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

rtla-symbols-test: rtla-build portable-build build
  node --test dist/tests/symbolic-operators/contract-test.js

apalache-test: build
  node --test dist/tests/apalache-comparison/contract-test.js

apalache-eval run java jar: portable-build build
  node dist/evals/experiments/apalache-comparison/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

backend-test: portable-build build
  pnpm exec vp test run --project=core tests/backend.vitest.test.ts
  node --test dist/tests/backend-contract/integration-test.js

backend-eval run: portable-build build
  node dist/evals/experiments/backend-contract/evaluate.js {{quote(run)}}

# Quint's generated TLA+ and replay against TLC.
quint-test: portable-build build
  node --test dist/tests/quint-generated/contract-test.js

analysis-test: portable-build rtla-build build
  cargo build --locked --manifest-path crates/rtla-compiler/Cargo.toml --example analysis_roles
  node --test dist/tests/tlc-differential/analysis-parity-test.js dist/tests/tlc-differential/frame-decoder-test.js dist/tests/p0-checker/contract-test.js dist/tests/prepared-safety/contract-test.js dist/tests/tla-coverage/contract-test.js dist/tests/tla-coverage/normalize-test.js dist/tests/rtla-compatibility/contract-test.js dist/tests/recursive-actions/contract-test.js dist/tests/frontend-semantics/contract-test.js

prepared-test: portable-build build
  cargo test --locked --manifest-path crates/idealis-core/Cargo.toml --test prepared_safety
  node --test dist/tests/prepared-safety/contract-test.js

prepared-eval before after run:
  node dist/evals/experiments/prepared-safety/measure.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

prepared-summary run output:
  node dist/evals/experiments/prepared-safety/summarize.js {{quote(run)}} {{quote(output)}}

# Freeze binaries first. These recipes do not rebuild Rust during measurement.
p0-corpus before after java jar archive run: build
  node dist/evals/experiments/p0-checker/corpus.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

p0-profile binary run: build
  node dist/evals/experiments/p0-checker/profile.js {{quote(binary)}} {{quote(run)}}

p0-comparison before after java jar archive run: build
  IDEALIS_COMPACT_ALL_MODELS=1 IDEALIS_COMPACT_PLAN=0062-p0-checker node dist/evals/experiments/compact-functions/compare.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

p0-verify run: build
  node dist/evals/experiments/p0-checker/verify.js {{quote(run)}}

evaluator-kernel before after run filter="":
  node dist/evals/experiments/oracle-tuning/kernel.js {{quote(before)}} {{quote(after)}} {{quote(run)}} safety-bench {{quote(filter)}}

evaluator-summary run output count plan="0063-evaluator-tuning":
  node dist/evals/experiments/evaluator-tuning/summarize.js {{quote(run)}} {{quote(output)}} {{quote(count)}} {{quote(plan)}}

evaluator-corpus before after java jar archive run plan="0063-evaluator-tuning":
  IDEALIS_CHECKER_PLAN={{quote(plan)}} IDEALIS_CHECKER_COMPARE_V2=1 node dist/evals/experiments/p0-checker/corpus.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

evaluator-verify snapshot run:
  IDEALIS_CHECKER_PLAN=0063-evaluator-tuning IDEALIS_CHECKER_SNAPSHOT={{quote(snapshot)}} node dist/evals/experiments/p0-checker/verify.js {{quote(run)}}

evaluator-corpus-summary run output plan="0063-evaluator-tuning":
  IDEALIS_CHECKER_PLAN={{quote(plan)}} node dist/evals/experiments/evaluator-tuning/corpus-summary.js {{quote(run)}} {{quote(output)}}

quint-generate output endpoint apalache_jar: build
  node dist/evals/experiments/quint-generated/generate.js {{quote(output)}} {{quote(endpoint)}} {{quote(apalache_jar)}}

quint-eval java tlc_jar apalache_jar run: portable-build build
  node dist/evals/experiments/quint-generated/run.js {{quote(run)}} {{quote(java)}} {{quote(tlc_jar)}} {{quote(apalache_jar)}}

quint-replay java tlc_jar apalache_jar saved run: portable-build build
  node dist/evals/experiments/quint-generated/run.js {{quote(run)}} {{quote(java)}} {{quote(tlc_jar)}} {{quote(apalache_jar)}} {{quote(saved)}}

# SMT expression and context expansion against Z3.
smt-expansion-test: smt-build build
  node --test dist/tests/smt-expansion/contract-test.js
  pnpm exec vp test run --project=smt

smt-expansion-eval phase run baseline parent="": smt-build build
  node dist/evals/experiments/smt-expansion/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}}

smt-expansion-replay run: build
  node dist/evals/experiments/smt-expansion/replay.js {{quote(run)}}

# SMT-LIB scripts, ordered queries and a shared work budget.
smt-script-test: smt-build build
  node --test dist/tests/smt-scripts/contract-test.js
  pnpm exec vp test run --project=smt

smt-cache-test: smt-build build
  node --test dist/tests/smt-cache/contract-test.js

# 0074: incremental contexts and the check cache against bcf018be1 and Z3.
smt-incremental-test: smt-build build
  node --test dist/tests/smt-incremental/records-test.js dist/tests/smt-incremental/contract-test.js

smt-incremental-bench-build:
  CARGO_PROFILE_BENCH_STRIP=none cargo bench --no-run --locked --manifest-path crates/rusmt/Cargo.toml --bench solver --bench cache --bench protocol

smt-incremental-capture binary bench library output: build
  node dist/evals/experiments/smt-incremental/capture.js {{quote(binary)}} {{quote(bench)}} {{quote(library)}} {{quote(output)}}

smt-incremental-criterion stage provenance run: build
  node dist/evals/experiments/smt-incremental/criterion.js {{quote(stage)}} {{quote(provenance)}} {{quote(run)}}

[positional-arguments]
smt-incremental-compare *args: build
  node dist/evals/experiments/smt-incremental/compare.js "$@"

smt-incremental-eval phase run baseline parent="" plan="": smt-build build
  node dist/evals/experiments/smt-incremental/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}} {{quote(plan)}}

smt-incremental-replay run: build
  node dist/evals/experiments/smt-incremental/replay.js {{quote(run)}}

smt-script-eval phase run baseline parent="": smt-build build
  node dist/evals/experiments/smt-scripts/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}}

smt-script-replay run: build
  node dist/evals/experiments/smt-scripts/replay.js {{quote(run)}}

# Fixed SMT benchmarks and immutable before/after binaries.
smt-tuning-build:
  CARGO_PROFILE_BENCH_STRIP=none cargo bench --no-run --locked --manifest-path crates/rusmt/Cargo.toml --bench solver

smt-tuning-capture binary output: build
  node dist/evals/experiments/smt-tuning/capture.js {{quote(binary)}} {{quote(output)}}

smt-tuning-criterion stage provenance run: build
  node dist/evals/experiments/smt-tuning/criterion.js {{quote(stage)}} {{quote(provenance)}} {{quote(run)}}

smt-tuning-compare before after confirmation after_before run: build
  node dist/evals/experiments/smt-tuning/compare.js {{quote(before)}} {{quote(after)}} {{quote(confirmation)}} {{quote(after_before)}} {{quote(run)}}

smt-tuning-eval phase run baseline parent="": smt-build build
  node dist/evals/experiments/smt-tuning/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}}

smt-tuning-replay run: build
  node dist/evals/experiments/smt-tuning/replay.js {{quote(run)}}

symmetry-test: safety-build
  cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test symmetry
  cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test partial_order
  node --test dist/tests/symmetry/contract-test.js

symmetry-prepare: safety-build
  node dist/evals/experiments/symmetry/prepare.js

symmetry-eval run java jar: safety-build
  node dist/evals/experiments/symmetry/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

# Build and freeze first; timing runs alone.
symmetry-benchmark run java jar archive binary:
  node dist/evals/experiments/symmetry/benchmark.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(binary)}}

symmetry-criterion binary run filter="":
  node dist/evals/experiments/symmetry/criterion.js {{quote(binary)}} {{quote(run)}} {{quote(filter)}}

# Use frozen executables; do not build during measurements.
symmetry-allocation before after run:
  node dist/evals/experiments/symmetry/allocation.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

symmetry-actions-eval run java jar: safety-build
  node dist/evals/experiments/symmetry/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} --actions

partial-order-eval run java jar: safety-build
  node dist/evals/experiments/symmetry/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} --por

symmetry-diagnose before after run model="linear-ring":
  node dist/evals/experiments/symmetry/diagnose.js {{quote(before)}} {{quote(after)}} {{quote(run)}} {{quote(model)}}

native-resources binary run:
  node dist/evals/experiments/native-resources/run.js {{quote(binary)}} {{quote(run)}}

symmetry-verify run:
  node dist/evals/experiments/symmetry/verify.js {{quote(run)}}

cache-proof output=".artifacts/cache-proof": build
  node dist/evals/experiments/verified-cache/formal.js {{quote(output)}}

cache-test: portable-build build
  node --test dist/tests/verified-cache/contract-test.js
  just vitest --project=cache

quint-bench run: build
  node dist/evals/experiments/quint-generated/bench.js {{quote(run)}}

quint-bench-replay run: build
  node dist/evals/experiments/quint-generated/bench.js {{quote(run)}} --replay

cache-tlc java jar output: build
  node dist/evals/experiments/verified-cache/tlc.js {{quote(java)}} {{quote(jar)}} {{quote(output)}}

cache-eval output: portable-build build
  node dist/evals/experiments/verified-cache/run.js {{quote(output)}}

cache-replay saved output: portable-build build
  node dist/evals/experiments/verified-cache/run.js {{quote(output)}} {{quote(saved)}}

expansion-test: portable-build build
  node --test dist/tests/cached-expansions/contract-test.js
  just vitest --project=expansions

expansion-eval output: portable-build build
  node dist/evals/experiments/cached-expansions/run.js {{quote(output)}}

expansion-replay saved output: portable-build build
  node dist/evals/experiments/cached-expansions/run.js {{quote(output)}} {{quote(saved)}}

expansion-proof output probes: build
  node dist/evals/experiments/verified-cache/formal.js {{quote(output)}} {{quote(probes)}}

expansion-bench output baseline: build
  node dist/evals/experiments/cached-expansions/benchmark.js {{quote(output)}} {{quote(baseline)}}

expansion-bench-replay output: build
  node dist/evals/experiments/cached-expansions/benchmark-replay.js {{quote(output)}}

smt-incremental-equivalence full compact: build
  node dist/evals/experiments/smt-incremental/equivalence.js {{quote(full)}} {{quote(compact)}}

smt-protocol-test: smt-build build
  node --test dist/tests/smt-protocol/contract-test.js

smt-protocol-wasm-bench stage run: smt-build build
  node dist/evals/experiments/smt-protocol/wasm-bench.js {{quote(stage)}} {{quote(run)}}

smt-protocol-compare native-dev native-conf wasm-dev wasm-conf output: build
  node dist/evals/experiments/smt-protocol/compare.js {{quote(native-dev)}} {{quote(native-conf)}} {{quote(wasm-dev)}} {{quote(wasm-conf)}} {{quote(output)}}

smt-protocol-compare-store native-dev native-conf wasm-dev wasm-conf previous output: build
  node dist/evals/experiments/smt-protocol/compare-store.js {{quote(native-dev)}} {{quote(native-conf)}} {{quote(wasm-dev)}} {{quote(wasm-conf)}} {{quote(previous)}} {{quote(output)}}

[positional-arguments]
smt-incremental-pair *args: build
  node dist/evals/experiments/smt-incremental/pair.js "$@"
