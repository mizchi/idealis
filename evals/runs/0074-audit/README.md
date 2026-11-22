# 0074-audit

Audit record of the iteration 0074 evaluation (plan `0074-nesting-chains`). File contents are the evaluator's originals with host-specific paths and names replaced by placeholders (`<scratch>`, `<after>`, `<before>`, `<clone>`, `<tools>`, `<mise>`, `<home>`, `<user>`, `<host>`). Binaries, `before-src`, tool installs and build outputs are not copied.

The evaluator used short run names. Files keep those names inside their contents (for example the `input` fields of the two summary `result.json` files and the paths in logs). The directories and ledger run IDs use the names below; no reader requires the two to match inside file contents.

| Directory | Original name | `commands.tsv` rows |
| --- | --- | --- |
| `0074-corpus` | `runs/corpus-1` | `corpus-1` |
| `0074-corpus-summary` | `runs/corpus-1-summary` | `summary-corpus-1` |
| `0074-kernel` | `runs/kernel-2` | `kernel-2` |
| `0074-kernel-summary` | `runs/kernel-2-summary`, `runs/kernel-2-table.txt` | `summary-kernel-2` |
| `0074-tlc-smoke` | `runs/tlc-smoke` | `tlc-smoke` |
| `0074-tlc` | `runs/tlc` | `tlc-full` |
| `0074-quint-regen` | `runs/quint-regen` | `quint-regen-LongConjunction`, `quint-regen-LongIfChain` |
| `0074-paths` | `runs/paths` | `paths-before` (exit 1, log removed by the evaluator, see `logs/paths-before.log.note`), `paths-before-r2`, `paths-after` |
| `0074-ir-corpora` | `runs/ir-corpora` | `ir-corpora-inputs`, `ir-corpora-before`, `ir-corpora-after` |
| `0074-temporal-parity` | `runs/temporal-parity` | `temporal-parity` |
| `0074-stack` | `runs/stack` | `stack-build-*` and `stack-bisect-*` |
| `0074-evaluate` | `runs/evaluate` | `just-evaluate`, `just-replay-evaluation` |
| `0074-abba-1` | `runs/abba-1`, `abba-1-table.txt`, `criterion-summary.txt` | `bench-abba-1` |
| `0074-abba-2` | `runs/abba-2`, `abba-2-table.txt`, `criterion-summary.txt` | `bench-abba-2` |
| `0074-abba-3` | `runs/abba-3`, `abba-3-table.txt`, `criterion-summary.txt` | `bench-abba-3` |
| `0074-browser` | `runs/browser-test-results`, `runs/browser-test-results-r2`, logs `just-browser*` | `just-browser` (Chromium not installed, failed), `just-browser-r2` (rerun) |
| `0074-audit/kernel-1` | `runs/kernel-1`, `kernel-1-table.txt` | `kernel-1`, `summary-kernel-1` |

`kernel-1` is an invalid run: the bench filter passed as the recipe's fourth parameter matched no benchmark, so every run executed zero benchmarks (see `logs/kernel-1.log.note`). It is kept here and has no ledger line. `kernel-2` is the valid run.

`browser-test-results-r2` contained no files and is not represented by a directory.

Ledger lines appended to `evals/runs/ledger.jsonl` (the lines the official recipes appended, saved in `ledger-appended.diff` and `ledger-appended-final.diff`, with the run IDs renamed):

| Original runId | Ledger runId |
| --- | --- |
| `kernel-2-summary` | `0074-kernel-summary` |
| `corpus-1` | `0074-corpus` |
| `corpus-1-summary` | `0074-corpus-summary` |

Other contents: `commands.tsv` (every command with start, end, load and exit), `drivers/`, `logs/`, `start/` (state before the run), `end*` files (state after), `checks/` (final checks, `*.status` and `*.log`, plus the protected-file digests and git-status files), `snapshots/` (manifests and digests only), `stack-harness/` (harness source and lock files without the build target).
