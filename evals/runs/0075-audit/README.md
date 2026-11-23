# 0075-audit

Audit record of the iteration 0075 evaluation (plan `0075-nesting-chains-ir-eval`). File contents are the evaluator's originals with host-specific paths and names replaced by placeholders (`<scratch>`, `<after>`, `<before>`, `<oracle>`, `<red>`, `<tmp>`, `<mise>`, `<home>`, `<user>`). Binaries, tool installs, build outputs and logs are not copied; `reduced.json` lists the removed files.

Trees: `<before>` is a detached worktree at 9488851d6 that also received the new rtlc tests, the stack harness and the `nesting_chains` bench source (its `crates/rtlc/src` is unchanged, see `checks/protected-*.sha256` and `checks/red-tests.sha256`). `<red>` is a second worktree at 9488851d6 with only the new tests, fixtures and experiment drivers copied in, used for the TS and rtla-compiler Red runs. `<oracle>` is 9488851d6 with `../0075-oracle/oracle.patch`. `<after>` is the branch.

`commands.tsv` lists every command with start, end, load and exit. Failed or superseded runs are kept there:

| Rows | What happened |
| --- | --- |
| `red-rtlc-*` (exit 127) | The driver passed `env -u …` as one word; rerun as `*-r2`. |
| `freeze-*`, `freeze-*-r2`, `freeze-*-r3`, `copy-*-bench-to-deps`, `-r2` | The snapshot step needs the rtlc bench in `target/release/deps`; the first attempts ran before it was copied, and a failed copy left an empty directory. `freeze-*-r4` are the snapshots used. |
| `baseline-*-0075` (exit 1) | `outcome.ts` had no `bounded` outcome; added, rerun as `-r2`. |
| `baseline-*-0075-r2`, `expect-0075`, `paths-*`, `tlc-0075` | First 0075 inputs. 21 inputs used an operator before defining it, which SANY rejects (`superseded/tlc-r1-results.json`). The inputs were reordered and everything rerun (`-r3`, `paths-*-r2`, `tlc-0075-r2`); the expectations were unchanged. |
| `red-compiler-nesting_chains` | Interrupted with no exit when a host hold started (`logs/red-compiler-nesting_chains.note`); rerun as `-r2`. |
| `red-ts-nesting-chains` | Never started (same hold); `-r2` is the Red run. |
| `temporal-parity`, `-r2`, `safety-build-after` | Missing `roracle.wasm` and a mistyped `env`; `safety-build-after-r2` then `temporal-parity-r3`. |
| `bench-abba-1`, `bench-abba-2`, `kernel`, `corpus`, `temporal-parity-r3`, `paths-*-r2`, `stack-*-after` | Ran on the after binary before the analysis-cache fix. ABBA-1 and ABBA-2 found the reproducible `check` regression; `bench-probe-defs` (v2 validation reverted) and `bench-probe-key` (cache key fixed) isolated it (`superseded/probe-*-table.txt`, `superseded/abba-3-probe-table.txt` for `#[inline]` alone). After the fix the after side was rebuilt (`*-after-r2`, `freeze-after-r5`) and every after-dependent run repeated (`paths-after-r3`, `ir-corpora-after-r2`, `corpus-r2`, `temporal-parity-r4`, `kernel-r2`, `stack-*-after-r2`, `bench-abba-3`); results were identical except one stack value (`superseded/stack-r1-summary.md`). |
| `check-check` (exit 1) | Formatting of six new or changed files; fixed with `vp check --fix`, `check-check-r2` passed. |

Ledger: the recipes appended lines with short run IDs (`ledger-appended.diff`); the committed ledger has one line each for `0075-corpus`, `0075-corpus-summary` and `0075-kernel-summary` (the final runs).

Other contents: `drivers/` (the scripts behind `commands.tsv`; two unused copies of 0074 drivers, `analyze-paths.py` and `analyze-tlc.py`, are not kept), `logs/*.exit`, `checks/` (protected-source digests before and after, the Red test file digests, snapshot and bench manifests, the final git status).
