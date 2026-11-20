# The model shard receives these release binaries from the portable-build job.
set allow-duplicate-recipes
set working-directory := '..'

import '../justfile'

oracle-wire-build:
  test -x crates/roracle/target/release/roracle
  test -s crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm

portable-build: oracle-wire-build
  test -x crates/idealis-core/target/release/idealis-core
  test -s crates/idealis-core/target/wasm32-unknown-unknown/release/idealis_core.wasm
