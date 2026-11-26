# 0119-audit

Audit record of the iteration 0119 evaluation (plan `0119-call-depth-limit`). File contents are the evaluator's originals with host-specific paths and names replaced by placeholders (`<scratch>`, `<after>`, `<before>`, `<oracle>`, `<mise>`, `<home>`, `<user>`). Binaries, tool installs, build outputs and the logs of builds are not copied.

Trees: `<before>` is `git archive` of the plan commit (origin/main 42b88c5a9 plus the plan) that also received the new rtlc and rtla-compiler tests and the stack harness (`crates/rtlc/src` is unchanged except for `safety_definition_tests.rs`, see `checks/protected-*.txt`). `<oracle>` is the same tree with `../0119-oracle/oracle.patch`. `<after>` is the branch.

`commands.tsv` lists every command with start, end, load and exit. Failed or superseded runs are kept there:

| Rows | What happened |
| --- | --- |
| `red-rtlc-evaluation_stack-debug`, `-release` (exit 127) | The driver passed `env -u ...` as one word; rerun as `-r2`. |
| `red-*`, `green-*`, `release-*`, `stack-*`, `check-rtlc-test`, `check-compiler-test` (and `-r2`), `red-wasm-*`, `red-ts-*`, `red-compiler-*` | The first value (512) of the plan. The tests and the product constant were changed to 256 after the Wasm measurement below. Their logs are kept; `superseded/stack-512-*.json` are the stack measurements of that value. |
| `check-rtlc-test` (exit 1), `check-compiler-test` (exit 1) | The ENABLED unit test overflowed the 2 MiB test thread in debug (fixed with an 8 MiB thread) and `cargo fmt` rejected `nesting_chains.rs`; `-r2` passed. |
| `wasm-oracle-node-*`, `wasm-oracle-node-*-r2`, `wasm-oracle-chromium`, `wasm-oracle-chromium-r2`, `wasm-oracle-smoke` | The Wasm measurement that decided the value. The first runs passed all cells as one argument (zsh does not split) and failed; the Chromium run printed its cells but the output file was taken by the Node driver that the browser driver imported (an unguarded entry point); `-r2` are the valid first-value measurements (`superseded/wasm-first-*`). They used the first oracle build, whose recording line sat after the increment. |
| `oracle-r2-build-*`, `wasm-oracle-r2-*` | The oracle rebuilt from the final patch, and the same Wasm measurement. The minimum depths differ slightly from the first build (update 1,553 against 1,237 on Node, 827 against 806 in the Worker), and the conclusion is the same (`../0119-wasm-ceiling/summary.md`). |
| `baseline-*-0119`, `-r2` | Baselines for the input list of the first value (512). `-r3`: the list for 256 before two input corrections; `-r4`: final. The corrections: a recursive call with a constant argument is folded before evaluation (`Sum(2)` completes at depth 155), so the input calls `Sum(x)`; the `UNCHANGED Vn` depths were declared from the oracle (n = 8 reaches 256). `expect-0119`, `-r2`, `-r3` are the matching fixture generations. |
| `green-ts-call-depth` (exit 1, in `green-ts-call-depth-r1.*`) | The two inputs above. The first attempt of the 4-path stage was interrupted and its logs were renamed `-r1`; the rerun passed 35 of 35. |
| `red2-wasm-callDepth` (exit 0) | `just wasm-build` rebuilt the Wasm core from the branch (`build-wasm-tests` builds Rust), so this rerun is not a Red. The Red of the Wasm host is `red-wasm-callDepth` (first value, before core). |
| `red3-parallel_stack-sets-debug-limit-only` (exit 0) | With the limit change and without the pool stack, the membership model is accepted: its 2 MiB worker limit and the depth 255 coincide within one level (about 8 KiB). The pool stack is therefore made Red by the unit test `workers_run_on_a_stack_larger_than_the_default` (`red4-workers-stack-unit-*`: abort in debug and release; `green4-*`: pass). `red-rtlc-parallel_stack-debug-limit-only` is the first value's Red. |
| `red3-*`, `green3-*`, `release3-*`, `check-*-r3`, `stack256-*` | The final value (256): Red on the before tree, Green on the branch (debug and release), full rtlc and rtla-compiler suites, the stack stage. |
| `cli-*` | The CLI Wasm (wasm32-wasip1, `node:wasi`) cannot reach the v2 evaluation limit: it has no IR version option and emits v1, whose frontend refuses 17 layers (`expression or operator nesting exceeds 40`). Both builds give the same refusal. |
| `size-before`, `size-after` | `size_of::<EvalContext>()` with a temporary probe test (72 bytes in both). The probe was removed. |
| `final-*` | The checks of the plan. `final-check` and `final-mutation` were run once before the last input edits and again as `-r2`. |

Ledger: the corpus recipes appended lines with the short run IDs `corpus` and `corpus-summary` and the kernel summary recipe is recorded as `0119-kernel-summary`; the committed ledger has one line each for `0119-corpus`, `0119-corpus-summary` and `0119-kernel-summary`, and the two lines of `just eval-loop` (`0119-loop-development`, `0119-loop-confirmation`).

Other contents: `drivers/` (the scripts behind `commands.tsv`), `logs/*.exit` and the logs of the Red, Green, final and measurement commands (the longest ones are cut to their last 60 lines), `checks/` (protected-source digests before and after, snapshot and tool digests, the final git status), `superseded/`.
