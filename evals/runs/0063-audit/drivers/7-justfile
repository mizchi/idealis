default:
  @just --list

support-check:
  node --experimental-strip-types scripts/check-support.ts

support-check-staged:
  node --experimental-strip-types scripts/check-support.ts --staged

support-plan:
  node --experimental-strip-types scripts/check-support.ts --dry-run

support-sync:
  node --experimental-strip-types scripts/check-support.ts --sync

support-sync-staged:
  node --experimental-strip-types scripts/check-support.ts --sync --staged

support-hooks:
  node --experimental-strip-types scripts/check-support.ts --install-hook

support-test:
  pnpm build
  node --test dist/scripts/check-support-test.js

support-eval run:
  pnpm build
  node dist/evals/crate-checklists/run.js {{quote(run)}}

support-replay run:
  pnpm build
  node dist/evals/crate-checklists/run.js {{quote(run)}} --replay

oracle-wire-build:
  pnpm oracle:build

workspace-oracle-build: oracle-wire-build
  cargo build --release --locked --manifest-path evals/workspace-oracle/rust/Cargo.toml

workspace-oracle-test: workspace-oracle-build portable-build testing-build ralloy-build rsmt-build rtla-build
  cargo fmt --manifest-path evals/workspace-oracle/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/workspace-oracle/rust/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path evals/workspace-oracle/rust/Cargo.toml
  pnpm build
  node --test dist/evals/workspace-oracle/contract-test.js dist/evals/workspace-oracle/catalog-test.js

# Run builds and tests first; snapshots freeze their executables and sources.
workspace-snapshot directory:
  pnpm build
  node dist/evals/workspace-oracle/tune.js snapshot {{quote(directory)}}

workspace-bench baseline candidate run:
  node dist/evals/workspace-oracle/tune.js run {{quote(baseline)}} {{quote(candidate)}} {{quote(run)}}

workspace-replay run replay:
  node dist/evals/workspace-oracle/tune.js replay {{quote(run)}} {{quote(replay)}}

workspace-oracle-cost run:
  node dist/evals/workspace-oracle/cost.js {{quote(run)}}

workspace-oracle-cost-replay run replay:
  node dist/evals/workspace-oracle/cost.js {{quote(replay)}} {{quote(run)}}

# Build and test first. Frozen binaries keep compilation outside the measurement.
oracle-tuning-freeze directory:
  node dist/evals/oracle-tuning/run.js freeze {{quote(directory)}}

oracle-tuning-measure before transport after run:
  node dist/evals/oracle-tuning/run.js measure {{quote(before)}} {{quote(transport)}} {{quote(after)}} {{quote(run)}}

oracle-tuning-compare before wire combined run:
  node dist/evals/oracle-tuning/run.js compare {{quote(before)}} {{quote(wire)}} {{quote(combined)}} {{quote(run)}}

oracle-tuning-phases snapshot run mode="measure":
  node dist/evals/oracle-tuning/phases.js {{quote(snapshot)}} {{quote(run)}} {{quote(mode)}}

oracle-tuning-compat before after run:
  node dist/evals/oracle-tuning/compat.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

oracle-tuning-kernel before after run:
  node dist/evals/oracle-tuning/kernel.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

oracle-criterion:
  cargo bench --locked --manifest-path crates/oracle/Cargo.toml --bench wire

oracle-build:
  cargo build --release --locked --manifest-path crates/oracle/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/oracle/Cargo.toml
  cargo build --release --locked --manifest-path evals/oracle-comparison/rust/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path evals/oracle-comparison/rust/Cargo.toml

oracle-test: oracle-build workspace-oracle-test
  cargo fmt --manifest-path crates/oracle/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/oracle/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path crates/oracle/Cargo.toml
  cargo test --locked --manifest-path crates/oracle/Cargo.toml --bench wire
  cargo fmt --manifest-path evals/oracle-comparison/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/oracle-comparison/rust/Cargo.toml -- -D warnings
  cargo test --locked --manifest-path evals/oracle-comparison/rust/Cargo.toml
  cargo run --example comparison --locked --manifest-path crates/oracle/Cargo.toml
  pnpm exec playwright test --config oracle-playwright.config.ts

# Build and run other checks before this recipe; measurements run alone.
oracle-eval run java jar:
  pnpm build
  node dist/evals/oracle-comparison/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

oracle-replay run:
  pnpm build
  node dist/evals/oracle-comparison/replay.js {{quote(run)}}

fuzz-build: testing-build
  cargo build --release --locked --manifest-path evals/structured-fuzz/rust/Cargo.toml
  cargo +nightly-2026-08-27 metadata --locked --format-version 1 --manifest-path evals/structured-fuzz/fuzz/Cargo.toml > /dev/null
  cargo +nightly-2026-08-27 fuzz build --sanitizer address --fuzz-dir evals/structured-fuzz/fuzz

fuzz-test: fuzz-build
  cargo fmt --manifest-path evals/structured-fuzz/rust/Cargo.toml -- --check
  cargo fmt --manifest-path evals/structured-fuzz/fuzz/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/structured-fuzz/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/structured-fuzz/rust/Cargo.toml
  pnpm build
  node --test dist/evals/structured-fuzz/contract-test.js
  pnpm exec vitest run --config fuzz-vitest.config.ts

fuzz-browser:
  pnpm exec playwright test --config fuzz-playwright.config.ts

fuzz-eval phase run parent="": fuzz-build
  pnpm build
  node dist/evals/structured-fuzz/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

fuzz-replay run:
  pnpm build
  node dist/evals/structured-fuzz/replay.js {{quote(run)}}

controlled-build: testing-build
  cargo build --release --locked --manifest-path evals/controlled-schedules/rust/Cargo.toml
  cargo build --release --locked --example loom --manifest-path evals/controlled-schedules/rust/Cargo.toml

controlled-test: controlled-build testing-core-test
  cargo fmt --manifest-path evals/controlled-schedules/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/controlled-schedules/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/controlled-schedules/rust/Cargo.toml
  cargo run --release --locked --example loom --manifest-path evals/controlled-schedules/rust/Cargo.toml
  pnpm build
  node --test dist/evals/controlled-schedules/contract-test.js
  pnpm exec vitest run --config schedules-vitest.config.ts

controlled-browser: controlled-build
  pnpm exec playwright test --config schedules-playwright.config.ts

controlled-eval phase run parent="": controlled-build
  pnpm build
  node dist/evals/controlled-schedules/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

controlled-replay run:
  pnpm build
  node dist/evals/controlled-schedules/replay.js {{quote(run)}}

sat-build:
  cargo build --release --locked --manifest-path evals/sat-foundation/rust/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path evals/sat-foundation/rust/Cargo.toml

sat-test: sat-build
  cargo fmt --manifest-path evals/sat-foundation/rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/sat-foundation/rust/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path evals/sat-foundation/rust/Cargo.toml
  pnpm build
  node --test dist/evals/sat-foundation/contract-test.js

sat-browser: sat-build
  pnpm exec playwright test --config sat-playwright.config.ts

sat-eval phase run parent="": sat-build
  pnpm build
  node dist/evals/sat-foundation/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

sat-replay run:
  pnpm build
  node dist/evals/sat-foundation/replay.js {{quote(run)}}

testing-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/idealis-testing/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/idealis-testing/Cargo.toml

testing-core-test:
  cargo fmt --manifest-path crates/idealis-testing/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/idealis-testing/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/idealis-testing/Cargo.toml

testing-example:
  cargo run --example counter --locked --manifest-path crates/idealis-testing/Cargo.toml

testing-test: testing-build
  pnpm build
  node --test dist/evals/testing-core/contract-test.js
  pnpm exec vitest run --config testing-vitest.config.ts

testing-browser: testing-build
  pnpm exec playwright test --config testing-playwright.config.ts

testing-eval phase run parent="": testing-build
  pnpm build
  node dist/evals/testing-core/run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

testing-replay run:
  pnpm build
  node dist/evals/testing-core/replay.js {{quote(run)}}

ralloy-build: oracle-wire-build
  cargo build --release --locked --manifest-path crates/ralloy/Cargo.toml
  cargo build --release --locked --lib --target wasm32-unknown-unknown --manifest-path crates/ralloy/Cargo.toml

ralloy-test:
  cargo fmt --manifest-path crates/ralloy/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/ralloy/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/ralloy/Cargo.toml

ralloy-example:
  cargo run --example graph --locked --manifest-path crates/ralloy/Cargo.toml

ralloy-contract: ralloy-build
  pnpm build
  node --test dist/evals/alloy-subset/contract-test.js

ralloy-browser: ralloy-build
  pnpm exec playwright test --config alloy-playwright.config.ts

ralloy-eval phase run java jar parent="": ralloy-build
  pnpm build
  node dist/evals/alloy-subset/run.js {{quote(phase)}} {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(parent)}}

ralloy-replay run:
  pnpm build
  node dist/evals/alloy-subset/replay.js {{quote(run)}}

alloy-test: ralloy-build
  pnpm build
  node --test dist/evals/alloy-instance/contract-test.js
  pnpm exec vitest run --config alloy-vitest.config.ts

alloy-browser: ralloy-build
  pnpm exec playwright test --config alloy-instance-playwright.config.ts

alloy-instance-eval phase run java jar parent="": ralloy-build
  pnpm build
  node dist/evals/alloy-instance/run.js {{quote(phase)}} {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(parent)}}

alloy-instance-replay run:
  pnpm build
  node dist/evals/alloy-instance/replay.js {{quote(run)}}

rsmt-build: oracle-wire-build
  pnpm rsmt:build

rsmt-test:
  pnpm rsmt:test

rsmt-example:
  cargo run --example ttl --locked --manifest-path crates/idealis-smt/Cargo.toml

rsmt-compare run baseline: rsmt-build
  pnpm build
  node dist/evals/rsmt-extraction/compare.js check {{quote(run)}} {{quote(baseline)}}

rsmt-criterion:
  cargo bench --locked --manifest-path crates/idealis-smt/Cargo.toml --bench solver

smt-build: rsmt-build

smt-test: rsmt-test

smt-criterion: rsmt-criterion

smt-verify phase run parent="": smt-build
  pnpm build
  node dist/evals/smt-subset/verify-run.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

smt-bench run z3: smt-build
  pnpm build
  node dist/evals/smt-subset/benchmark.js {{quote(run)}} {{quote(z3)}}

smt-browser: smt-build
  pnpm exec playwright test --config smt-playwright.config.ts

install:
  pnpm install --frozen-lockfile

check:
  pnpm check
  pnpm typecheck
  pnpm test

test:
  pnpm test

mutation:
  pnpm test:mutation

browser: portable-build
  pnpm test:browser

demo:
  pnpm example

evaluate:
  pnpm evaluate

replay-evaluation:
  pnpm evaluate:replay

eval-loop:
  pnpm eval:loop evals/plans/0002-mixed-depth.json

replay-loop:
  pnpm eval:replay

runtime-build:
  pnpm runtime:build

runtime-test:
  pnpm runtime:test
  pnpm runtime:browser

runtime-eval:
  pnpm runtime:eval

portable-build: oracle-wire-build
  pnpm portable:build

portable-test:
  pnpm portable:test
  pnpm portable:browser

portable-eval:
  pnpm portable:eval

portable-replay run:
  pnpm portable:replay {{quote(run)}}

safety-build: portable-build
  pnpm build

safety-test: safety-build
  node --test dist/evals/rust-safety/contract-test.js dist/evals/collections-temporal/contract-test.js dist/evals/tlc-differential/compare-test.js dist/evals/tlc-differential/regression-test.js

safety-eval run plan="0014-value-hash": safety-build
  node dist/evals/rust-safety/evaluate.js {{quote(run)}} {{quote(plan)}}

safety-replay run:
  pnpm build
  node dist/evals/rust-safety/replay.js {{quote(run)}}

safety-tlc java jar: safety-build
  node dist/evals/rust-safety/tlc-check.js {{quote(java)}} {{quote(jar)}}

go-test: portable-build
  pnpm build
  node dist/evals/go/prepare.js
  test -z "$(gofmt -l adapters/go)"
  cd adapters/go && go vet ./...
  cd adapters/go && CGO_ENABLED=0 go test -count=1 ./...
  cd adapters/go && go test -race -count=1 ./...
  node dist/scripts/go-consumer.js

cross-build: portable-build
  pnpm build
  cargo build --release --locked --manifest-path evals/shared-oracle-rust/Cargo.toml
  mkdir -p .artifacts/go
  cd adapters/go && CGO_ENABLED=0 go build -o ../../.artifacts/go/counter-replay ./cmd/counter-replay

cross-test: cross-build
  cargo fmt --manifest-path evals/shared-oracle-rust/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path evals/shared-oracle-rust/Cargo.toml -- -D warnings
  node --test dist/evals/cross-language/contract-test.js

cross-eval phase run parent="": cross-build
  node dist/evals/cross-language/evaluate.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

cross-replay run:
  pnpm build
  node dist/evals/cross-language/replay.js {{quote(run)}}

async-build: cross-build
  cd adapters/go && CGO_ENABLED=0 go build -o ../../.artifacts/go/history-check ./cmd/history-check
  cd adapters/go && CGO_ENABLED=0 go build -o ../../.artifacts/go/finite-bench ./cmd/finite-bench

async-test: async-build
  node --test dist/evals/async-tuning/contract-test.js

async-eval phase run parent="": async-build
  node dist/evals/async-tuning/evaluate.js {{quote(phase)}} {{quote(run)}} {{quote(parent)}}

async-replay run:
  pnpm build
  node dist/evals/async-tuning/replay.js {{quote(run)}}

usable-test:
  pnpm usable:test

usable-eval:
  pnpm usable:eval

usable-replay run:
  pnpm usable:replay {{quote(run)}}

usable-benchmark input output:
  pnpm build
  node dist/evals/usable/benchmark-run.js {{quote(input)}} {{quote(output)}}

package:
  pnpm package:prepare
  pnpm package:test

complex-tlc java jar run: safety-build
  node dist/evals/collections-temporal/tlc-check.js {{quote(java)}} {{quote(jar)}} {{quote(run)}}

complex-eval run parent="": safety-build
  node dist/evals/collections-temporal/evaluate.js {{quote(run)}} {{quote(parent)}}

complex-replay run:
  pnpm build
  node dist/evals/collections-temporal/replay.js {{quote(run)}}

complex-tlc-replay java jar run:
  pnpm build
  node dist/evals/collections-temporal/tlc-replay.js {{quote(java)}} {{quote(jar)}} {{quote(run)}}


tlc-modules:
  pnpm build
  node dist/scripts/prepare-tlc-modules.js

tlc-diff-test java jar: safety-build tlc-modules
  node --test dist/evals/tlc-differential/compare-test.js dist/evals/tlc-differential/standard-modules-test.js
  TLC_JAVA={{quote(java)}} TLC_JAR={{quote(jar)}} node --test dist/evals/tlc-differential/standard-modules-tlc-test.js


tlc-diff java jar phase run: safety-build tlc-modules
  node dist/evals/tlc-differential/run.js {{quote(java)}} {{quote(jar)}} {{quote(phase)}} {{quote(run)}}

tlc-diff-replay java jar saved run: safety-build tlc-modules
  node dist/evals/tlc-differential/run.js {{quote(java)}} {{quote(jar)}} replay {{quote(run)}} {{quote(saved)}}

checker-bench-test:
  pnpm build
  node --test dist/evals/checker-benchmark/contract-test.js

checker-bench java jar archive run: safety-build
  node dist/evals/checker-benchmark/run.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

checker-bench-replay java jar archive saved run: safety-build
  node dist/evals/checker-benchmark/replay.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(saved)}} {{quote(run)}}

parallel-safety-test: safety-build
  node --test dist/evals/parallel-safety/contract-test.js

parallel-safety-eval run: safety-build
  node dist/evals/parallel-safety/evaluate.js {{quote(run)}}

parallel-safety-replay saved run: safety-build
  node dist/evals/parallel-safety/replay.js {{quote(saved)}} {{quote(run)}}

tlc-throughput java jar archive run: safety-build
  node dist/evals/tlc-throughput/run.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

tlc-throughput-replay java jar archive saved run: safety-build
  node dist/evals/tlc-throughput/run.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

single-model-test java jar: safety-build
  IDEALIS_TLC_JAVA={{quote(java)}} IDEALIS_TLC_JAR={{quote(jar)}} node --test dist/evals/single-model/contract-test.js dist/evals/single-model/integration-test.js

single-model-eval java jar archive run: safety-build
  node dist/evals/single-model/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

single-model-replay java jar archive saved run: safety-build
  node dist/evals/single-model/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

frontier-safety-test: safety-build
  cargo test --all-targets --locked --manifest-path crates/idealis-core/Cargo.toml
  node --test dist/evals/frontier-safety/contract-test.js

frontier-safety-eval java jar archive run: safety-build
  node dist/evals/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

frontier-safety-replay java jar archive saved run: safety-build
  node dist/evals/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

frontier-budget-eval java jar archive run: safety-build
  IDEALIS_FRONTIER_PLAN=0021-worker-budget node dist/evals/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

frontier-budget-replay java jar archive saved run: safety-build
  IDEALIS_FRONTIER_PLAN=0021-worker-budget node dist/evals/frontier-safety/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

readme-bench-test java jar: safety-build
  IDEALIS_TLC_JAVA={{quote(java)}} IDEALIS_TLC_JAR={{quote(jar)}} node --test dist/evals/readme-comparison/contract-test.js dist/evals/readme-comparison/integration-test.js

readme-bench java jar archive run: safety-build
  node dist/evals/readme-comparison/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

readme-bench-replay java jar archive saved run: safety-build
  node dist/evals/readme-comparison/evaluate.js {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}} {{quote(saved)}}

criterion-prepare:
  pnpm build
  node dist/evals/criterion-safety/prepare.js

criterion-test: criterion-prepare
  cargo test --locked --manifest-path crates/rtlc/Cargo.toml --bench safety
  node --test dist/evals/criterion-safety/contract-test.js

criterion-save stage run: criterion-prepare
  node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

criterion-controls run: safety-build
  node dist/evals/criterion-safety/compare.js {{quote(run)}} controls

criterion-compare run: safety-build
  node dist/evals/criterion-safety/compare.js {{quote(run)}}

criterion-replay saved run:
  pnpm build
  node dist/evals/criterion-safety/compare.js {{quote(run)}} {{quote(saved)}}

profile-criterion stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0024-profile-tuning node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

profile-record backend binary run:
  pnpm build
  node dist/evals/profile-tuning/record.js {{quote(backend)}} {{quote(binary)}} {{quote(run)}}

profile-test: criterion-prepare
  node --test dist/evals/profile-tuning/hotspots-test.js dist/evals/profile-tuning/wasm-names-test.js

profile-summary saved run:
  pnpm build
  node dist/evals/profile-tuning/summarize.js {{quote(saved)}} {{quote(run)}}

profile-scalar run: safety-build
  node dist/evals/profile-tuning/scalar-compare.js {{quote(run)}}

profile-replay saved run:
  pnpm build
  node dist/evals/profile-tuning/scalar-compare.js {{quote(run)}} {{quote(saved)}}

rtlc-test:
  pnpm rtlc:test

rtlc-example:
  cargo run --example counter --locked --manifest-path crates/rtlc/Cargo.toml

compact-functions-freeze directory:
  node dist/evals/compact-functions/freeze.js {{quote(directory)}}

compact-functions-compare before after java jar archive run:
  node dist/evals/compact-functions/compare.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

compact-functions-kernel before after run:
  node dist/evals/oracle-tuning/kernel.js {{quote(before)}} {{quote(after)}} {{quote(run)}} safety-bench

compact-functions-summary comparison criterion run:
  node dist/evals/compact-functions/summarize.js {{quote(comparison)}} {{quote(criterion)}} {{quote(run)}}

rtlc-compare run:
  pnpm build
  node dist/evals/rtlc-extraction/compare.js {{quote(run)}}

rtla-build: oracle-wire-build
  pnpm rtla:build

compiler-test:
  cargo fmt --manifest-path crates/rtla-compiler/Cargo.toml -- --check
  cargo clippy --all-targets --locked --manifest-path crates/rtla-compiler/Cargo.toml -- -D warnings
  cargo test --all-targets --locked --manifest-path crates/rtla-compiler/Cargo.toml

coverage-test: safety-build
  node --test dist/evals/action-coverage/contract-test.js

coverage-eval run java jar: safety-build
  node dist/evals/action-coverage/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

coverage-replay saved run java jar: safety-build
  node dist/evals/action-coverage/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

coverage-controls stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0050-action-coverage node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

coverage-bench stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0050-action-coverage IDEALIS_CRITERION_TARGET=rtlc-coverage node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

module-eval run java jar: safety-build
  node dist/evals/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

module-replay saved run java jar: safety-build
  node dist/evals/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

operators-eval run java jar: safety-build
  IDEALIS_TLC_SUITE=fairness-operators node dist/evals/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

operators-replay saved run java jar: safety-build
  IDEALIS_TLC_SUITE=fairness-operators node dist/evals/module-linking/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

tlc-finite-eval run java jar: safety-build
  node dist/evals/tlc-finite/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

tlc-finite-replay saved run java jar: safety-build
  node dist/evals/tlc-finite/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(saved)}}

compiler-criterion stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0048-fairness-operators IDEALIS_CRITERION_TARGET=rtla-compiler node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

temporal-profile run:
  pnpm build
  cargo build --release --locked --manifest-path crates/rtla-compiler/Cargo.toml --example temporal-profile
  node dist/evals/temporal-tuning/profile.js crates/rtla-compiler/target/release/examples/temporal-profile {{quote(run)}}

temporal-criterion stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0049-temporal-tuning IDEALIS_CRITERION_TARGET=rtla-compiler node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

temporal-controls stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0049-temporal-tuning node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

temporal-parity baseline run: safety-build
  node dist/evals/temporal-tuning/parity.js {{quote(baseline)}} {{quote(run)}}

temporal-compare before after run:
  pnpm build
  node dist/evals/temporal-tuning/compare.js {{quote(before)}} {{quote(after)}} {{quote(run)}}

temporal-alternating run:
  pnpm build
  node dist/evals/temporal-tuning/alternating.js {{quote(run)}}

criterion-baseline revision stage run: criterion-prepare
  IDEALIS_CRITERION_PLAN=0046-tlc-finite-support IDEALIS_CRITERION_REVISION={{quote(revision)}} node dist/evals/criterion-safety/run.js {{quote(stage)}} {{quote(run)}}

criterion-verify run:
  pnpm build
  node dist/evals/criterion-safety/verify.js {{quote(run)}}

compiler-eval run: portable-build
  pnpm build
  node dist/evals/rtla-compiler/evaluate.js {{quote(run)}}

compiler-tlc run java jar: portable-build
  pnpm build
  node dist/evals/rtla-compiler/tlc.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

rtla-test:
  pnpm rtla:test

rtla-browser: rtla-build
  pnpm exec playwright test --config rtla-playwright.config.ts

rtla-eval run java jar: rtla-build
  pnpm build
  node dist/evals/rtla-parser/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

apalache-test:
  pnpm build
  node --test dist/evals/apalache-comparison/contract-test.js

apalache-eval run java jar: portable-build
  pnpm build
  node dist/evals/apalache-comparison/run.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

backend-test: portable-build
  pnpm build
  pnpm exec vitest run tests/backend.vitest.test.ts
  node --test dist/evals/backend-contract/integration-test.js

backend-eval run: portable-build
  pnpm build
  node dist/evals/backend-contract/evaluate.js {{quote(run)}}

# Quint's generated TLA+ and replay against TLC.
quint-test: portable-build
  pnpm build
  node --test dist/evals/quint-generated/contract-test.js

analysis-test: portable-build rtla-build
  cargo build --locked --manifest-path crates/rtla-compiler/Cargo.toml --example analysis_roles
  pnpm build
  node --test dist/evals/tlc-differential/analysis-parity-test.js dist/evals/tlc-differential/frame-decoder-test.js dist/evals/p0-checker/contract-test.js

# Freeze binaries first. These recipes do not rebuild Rust during measurement.
p0-corpus before after java jar archive run:
  pnpm build
  node dist/evals/p0-checker/corpus.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

p0-profile binary run:
  pnpm build
  node dist/evals/p0-checker/profile.js {{quote(binary)}} {{quote(run)}}

p0-comparison before after java jar archive run:
  pnpm build
  IDEALIS_COMPACT_ALL_MODELS=1 IDEALIS_COMPACT_PLAN=0062-p0-checker node dist/evals/compact-functions/compare.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

p0-verify run:
  pnpm build
  node dist/evals/p0-checker/verify.js {{quote(run)}}

evaluator-kernel before after run filter="":
  node dist/evals/oracle-tuning/kernel.js {{quote(before)}} {{quote(after)}} {{quote(run)}} safety-bench {{quote(filter)}}

evaluator-summary run output count:
  node dist/evals/evaluator-tuning/summarize.js {{quote(run)}} {{quote(output)}} {{quote(count)}}

evaluator-corpus before after java jar archive run:
  IDEALIS_CHECKER_PLAN=0063-evaluator-tuning IDEALIS_CHECKER_COMPARE_V2=1 node dist/evals/p0-checker/corpus.js {{quote(before)}} {{quote(after)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(run)}}

evaluator-verify snapshot run:
  IDEALIS_CHECKER_PLAN=0063-evaluator-tuning IDEALIS_CHECKER_SNAPSHOT={{quote(snapshot)}} node dist/evals/p0-checker/verify.js {{quote(run)}}

evaluator-corpus-summary run output:
  node dist/evals/evaluator-tuning/corpus-summary.js {{quote(run)}} {{quote(output)}}

quint-generate output endpoint apalache_jar:
  pnpm build
  node dist/evals/quint-generated/generate.js {{quote(output)}} {{quote(endpoint)}} {{quote(apalache_jar)}}

quint-eval java tlc_jar apalache_jar run: portable-build
  pnpm build
  node dist/evals/quint-generated/run.js {{quote(run)}} {{quote(java)}} {{quote(tlc_jar)}} {{quote(apalache_jar)}}

quint-replay java tlc_jar apalache_jar saved run: portable-build
  pnpm build
  node dist/evals/quint-generated/run.js {{quote(run)}} {{quote(java)}} {{quote(tlc_jar)}} {{quote(apalache_jar)}} {{quote(saved)}}

# SMT expression and context expansion against Z3.
smt-expansion-test: smt-build
  pnpm build
  node --test dist/evals/smt-expansion/contract-test.js
  pnpm exec vitest run --config smt-vitest.config.ts

smt-expansion-eval phase run baseline parent="": smt-build
  pnpm build
  node dist/evals/smt-expansion/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}}

smt-expansion-replay run:
  pnpm build
  node dist/evals/smt-expansion/replay.js {{quote(run)}}

# SMT-LIB scripts, ordered queries and a shared work budget.
smt-script-test: smt-build
  pnpm build
  node --test dist/evals/smt-scripts/contract-test.js
  pnpm exec vitest run --config smt-vitest.config.ts

smt-script-eval phase run baseline parent="": smt-build
  pnpm build
  node dist/evals/smt-scripts/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}}

smt-script-replay run:
  pnpm build
  node dist/evals/smt-scripts/replay.js {{quote(run)}}

# Fixed SMT benchmarks and immutable before/after binaries.
smt-tuning-build:
  CARGO_PROFILE_BENCH_STRIP=none cargo bench --no-run --locked --manifest-path crates/idealis-smt/Cargo.toml --bench solver

smt-tuning-capture binary output:
  pnpm build
  node dist/evals/smt-tuning/capture.js {{quote(binary)}} {{quote(output)}}

smt-tuning-criterion stage provenance run:
  pnpm build
  node dist/evals/smt-tuning/criterion.js {{quote(stage)}} {{quote(provenance)}} {{quote(run)}}

smt-tuning-compare before after confirmation after_before run:
  pnpm build
  node dist/evals/smt-tuning/compare.js {{quote(before)}} {{quote(after)}} {{quote(confirmation)}} {{quote(after_before)}} {{quote(run)}}

smt-tuning-eval phase run baseline parent="": smt-build
  pnpm build
  node dist/evals/smt-tuning/run.js {{quote(phase)}} {{quote(run)}} {{quote(baseline)}} {{quote(parent)}}

smt-tuning-replay run:
  pnpm build
  node dist/evals/smt-tuning/replay.js {{quote(run)}}

symmetry-test: safety-build
  cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test symmetry
  cargo test --locked --manifest-path crates/rtla-compiler/Cargo.toml --test partial_order
  node --test dist/evals/symmetry/contract-test.js

symmetry-prepare: safety-build
  node dist/evals/symmetry/prepare.js

symmetry-eval run java jar: safety-build
  node dist/evals/symmetry/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}}

# Build and freeze first; timing runs alone.
symmetry-benchmark run java jar archive binary:
  node dist/evals/symmetry/benchmark.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} {{quote(archive)}} {{quote(binary)}}

symmetry-criterion binary run filter="":
  node dist/evals/symmetry/criterion.js {{quote(binary)}} {{quote(run)}} {{quote(filter)}}

symmetry-actions-eval run java jar: safety-build
  node dist/evals/symmetry/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} --actions

partial-order-eval run java jar: safety-build
  node dist/evals/symmetry/evaluate.js {{quote(run)}} {{quote(java)}} {{quote(jar)}} --por

symmetry-diagnose before after run model="linear-ring":
  node dist/evals/symmetry/diagnose.js {{quote(before)}} {{quote(after)}} {{quote(run)}} {{quote(model)}}

native-resources binary run:
  node dist/evals/native-resources/run.js {{quote(binary)}} {{quote(run)}}

symmetry-verify run:
  node dist/evals/symmetry/verify.js {{quote(run)}}
