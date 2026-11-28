# held steps: the 'before exit' recipes of the plan, run on the worktree (after = 0087a7ac4 plus the evals files)

| recipe | row in commands.tsv | exit | wall s | peak RSS KiB | counts from the log |
| --- | --- | ---: | ---: | ---: | --- |
| `just check` | s4-check | 0 | 6.9
 | 722640 | vp format 1097 files, lint 669 files with no warnings, tsc, vitest 79 tests in 14 files, node:test 79 pass |
| `just wasm-test` | s4-wasm-test | 0 | 88.8
 | 1021984 | node:test 24 pass, vitest 24 pass |
| `just safety-test` | s4-safety-test | 0 | 100.4
 | 2378816 | node:test 1817 pass 0 fail |
| `just portable-test` | s4-portable-test | 0 | 79.8
 | 2163536 | cargo fmt, clippy -D warnings and 750 cargo tests pass in rtlc (lib 165), rtla, rtla-compiler and idealis-core; node:test 3 + 22 + 9 pass |
| `just go-test (mise exec go@1.25.14, GOFLAGS=-buildvcs=false)` | s4-go-test | 0 | 130.0
 | 453136 | gofmt clean, go vet, go test 1 package ok, go test -race 1 package ok, go-consumer complete (cgo false, Wasm sha256 e22f914c...) |
| `just quint-test` | s4-quint-test | 0 | 1.6
 | 764576 | node:test 38 pass |
| `just evaluate evals/runs/0120-evaluate` | s4-evaluate | 0 | 1.6
 | 625520 | evidence written to evals/runs/0120-evaluate |
| `just replay-evaluation evals/runs/0120-evaluate/witnesses.json` | s4-replay-evaluation | 0 | 1.0
 | 600688 | 5 faults reproduced (persistence, deduplication, late-retry, fifo-only, drop-ignored) and the fixed implementation passed; 6 schedules replayed |
| `just eval-loop evals/plans/0002-mixed-depth.json 0120-loop` | s4-eval-loop | 0 | 3.0
 | 609472 | 0120-loop-development complete / improved; 0120-loop-confirmation complete / improved |
| `just replay-loop evals/runs/0120-loop-confirmation` | s4-replay-loop | 0 | 1.0
 | 601728 | 10 witnesses reproduced; normal controls passed |
| `just runtime-build` | s4-runtime-build | 0 | 0.1
 | 0 | up to date (release and wasm32 builds of evals/experiments/rust-probe) |
| `just runtime-test` | s4-runtime-test | 0 | 2.5
 | 493520 | cargo 2 pass; node:test 3 + 1 pass |

Each recipe ran under audit-0120/run.py (RSS cap 4 GiB, 30 min, disk floor 30 GiB); none hit a cap. Ledger lines written: 0120-corpus, 0120-corpus-summary, 0120-kernel-summary, 0120-loop-development, 0120-loop-confirmation (the run ids were passed explicitly, so no rename was needed; 0120-safety was renamed in the earlier stage). `just evaluate` writes no ledger line.

Included by the recipes above: just rtlc-test, just rtla-test and just compiler-test (cargo fmt --check, clippy -D warnings, cargo test) through just portable-test, and the release/debug Wasm and native builds through just portable-build. Not run in this stage: just mutation, just browser, just portable-browser (the earlier stage ran the two browser recipes), the release-profile cargo tests of evaluation_stack, parallel_stack and call_depth (the earlier stage ran them), and the Go adapter recipes beyond just go-test.
