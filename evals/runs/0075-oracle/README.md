# 0075-oracle

The observation-only copy of the "before" source (9488851d6) with the checker's 48 and the non-recursive dynamic-evaluation 128 removed (`oracle.patch`; the recursive 128 stays, and the evaluation depth saturates). It was built with the same commands as "before" (`snapshot-oracle.json` holds its binaries' sha256). It is never used outside this evaluation.

`baseline-{before,oracle}-{0074,0075}*.json` are the outputs of `evals/experiments/nesting-chains/baseline.ts` for the 0074 inputs and the 0075 inputs. `-r3` are the outputs after the 0075 inputs were changed to define each operator before its use (the first 0075 outputs are kept without a suffix). The expectations in `fixtures/nesting-chains/cases.json` and `cases-0075.json` were derived from `baseline-*-0074.json` and `baseline-*-0075-r3.json` (copied into `fixtures/nesting-chains/records-0075.json`); the first 0075 outputs gave the same expectations.

The comparison of after with this copy is in `../0075-paths/analysis.txt` ("changed cells where after differs from the oracle").
