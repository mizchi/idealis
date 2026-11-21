# 0074-r2-audit

Audit record of the second evaluation of iteration 0074 (plan `0074-nesting-chains`): the parts of the first evaluation (`0074-audit` and its siblings) that the post-review fix can affect, re-run against the current worktree. The first evaluation's records are unchanged except one value: the host temporary directory in `0074-corpus/manifest.json` (`java.io.tmpdir`) was replaced by `<tmp>/` after the first review. In `0074-r2-fix`, `run.sh` truncates a log of the same name on each attempt, so for the two failed `sizes-record-base` attempts (rows 6 and 8 of its `commands.tsv`) only the exit codes are kept and the log holds the successful run. File contents carry the same placeholders as `0074-audit` (`<scratch>`, `<after>`, `<before>`, `<clone>`, `<tools>`, `<mise>`, `<home>`, `<user>`, `<host>`, `<tmp>`). Values, counts and sha256 are unchanged by the placeholders. Binaries and build outputs are not copied.

What changed after the first evaluation: (1) `actionChain` in `packages/idealis/src/tla-safety.ts` accumulates `Or` operands by push instead of copying the accumulated array; (2) group `limit` (25 cases) was added to `fixtures/nesting-chains/{baseline,cases,digests}.json` and `sizes.json`, generated from the base `0c425fdaa`, with new tests; (3) `extracted` in `fixtures/tlc-operator-sharing/analysis-inventory.json` was re-extracted. The Rust frontend source is unchanged (see `source-hashes.txt`).

| Directory | Step | `commands.tsv` rows |
| --- | --- | --- |
| `0074-r2-audit` | 1. rebuild of the after artefacts, hashes | `build-after-ts`, `build-after-core`, `build-after-wasm` |
| `0074-r2-paths` | 2. four-path comparison (TS to Wasm, TS to native, Rust Wasm, Rust native; IR v1 and v2) over the 244 original inputs plus 25 `limit` cases | `paths-after-r2` |
| `0074-r2-ir-corpora` | 3. existing-input IR comparison (4,651 inputs, 27,906 cells) | `ir-corpora-inputs`, `ir-corpora-after-r2` |
| `0074-r2-browser` | 6. `just browser` | `just-browser-r2` |
| `0074-r2-sizes` | 7. size cases, three runs | `sizes-record-r2-1`, `-2`, `-3`, `size-cases-test-r2` |
| `0074-r2-fix` | evidence of the fix round (commands, logs, plants, measure scripts) | its own `commands.tsv` |

Steps 4 and 5 were not run:

- `just evaluator-corpus` (step 4): all 8 comparison paths (before and after, native and Wasm, IR v1 and v2) call `check-tla` in the Rust core snapshots. `corpus.ts` calls the TS frontend only to write `ts-frontend.json` (compile status, time and IR size), which no comparison reads. The core native and Wasm binaries rebuilt in this run have the same sha256 as the first evaluation's after snapshot (`after-artefacts.sha256`), and the Rust frontend source is unchanged, so every compared value is unaffected. No ledger line was appended.
- `just temporal-parity` (step 5): `parity.ts` imports only the Rust core (native and Wasm) and no TS frontend; the same binary identity applies. Not run, no ledger line.

The ledger `evals/runs/ledger.jsonl` was not modified by this evaluation (`ledger-before.sha256` and `ledger-after.sha256` are equal). `pnpm-lock.yaml` was restored with `git checkout -- pnpm-lock.yaml` after each `just` command (sha256 unchanged, `ad7a8659...`).

## Results

Rust frontend: `typed_expr.rs` and `typed_lower.rs` have the sha256 recorded after the first evaluation (`f801cd67...`, `42fd8301...`); the other `crates/rtla-compiler/src/*.rs` files are byte-identical to the base. `tla-safety.ts` changed from `55141a5c...` to `d114857b...`.

After artefacts: `idealis-core` `dc11f024403ec4ec719077c73f9f9d8588155d6844647b735fe68a0c739ad8b4` and `idealis_core.wasm` `c3eb1a0ed6510c75ddad9289fc715cfaea457404cb0e9887f7204ea0e7d38054`, both equal to the first evaluation's after snapshot. `dist/packages/idealis/src/tla-safety.js` is `93d357cf9006335aa31a41f8506e0b45324a54b80c01227cce9564285fa65adb`.

Four-path comparison (`0074-r2-paths/paths-analysis-r2.txt`): 269 cases (244 original, including 4 Quint products, plus 25 `limit`).

- first after vs after-r2 over the 244 original inputs: 1,952 cells (inputs x v1, v2 x 4 paths), 0 differing in `outcome`, `ir` or `irNormalized`.
- the 25 `limit` cases: 200 cells against `expect` in `cases.json`, 0 mismatches. All 2,120 non-Quint cells: 0 mismatches.
- exception cells: 0. Pre-existing and unchanged from the first evaluation: 16 (case, path) pairs where the IR of a spelling differs from its equivalent spelling (the first evaluation also reports 16), and one raw-IR split `enabled-and-flat-44` v1 Rust Wasm vs native (normalised IR equal).

Existing-input IR comparison (`0074-r2-ir-corpora/analysis-r2.txt`): the regenerated input list has 4,676 inputs, the 4,651 original inputs (source and config byte-identical) plus the 25 `limit` cases, so its `inputsSha256` is `259ea411...` (first: `4ca0bbe5...`). Over the 4,651 original inputs, 27,906 cells: before vs after-r2 identical 27,906, differing 0; first after vs after-r2 identical 27,906, differing 0. TS vs Rust-native normalised IR differing 89 of 8,319 and Rust Wasm vs native differing 0 of 8,451, both as in the first evaluation. The 25 `limit` inputs have no before value in the first evaluation and are listed separately in the analysis.

Browser (`0074-r2-browser`): `just browser` exit 0, 7 passed.

Sizes (`0074-r2-sizes`): outcomes of all three runs equal `fixtures/nesting-chains/sizes.json` byte for byte; the contract test's size cases pass (6 of 6). The 100,000-term case on ts, `elapsedMs` / `peakRssBytes` per run: 696 / 764,395,520; 672 / 763,019,264; 666 / 754,794,496. The first evaluation's base measurement of the same case in the fix round was 897 ms / 748,486,656 bytes (`0074-r2-fix/logs/sizes-record-base.log`, run on the base source); the limits are 5,000 ms and 1 GiB.
