Correction 0003 to the stage T freeze: a control that reaches `Snapshot::new` with a nonzero forwarded count (fault F5c)

Trigger
- The evaluation (evals/runs/0120-audit/faults/summary.md) found that fault F5c (`Snapshot::new` sets `forwarded: 0`) fails no lib test. The named control `v2_forwarded_depth_reaches_enabled_and_snapshots` never reaches `Snapshot::new`; the tests that do reach it with forwarded 69/121/129 give results independent of the value.
- Classification: Evidence. The design (two depth numbers, 128 checked against evaluation_depth + forwarded, snapshots resume from the sum) stands; only the control is missing.

Change (crates/rtlc/src/safety_definition_tests.rs only; one helper, one test, no existing test or product source touched)
- `applied_recursive_function_program(passes, body_calls)`: the shape of the evaluator's scratch probe (faults/F5c-proposed-control.rs.txt). A lazy function literal is passed through `passes` forwarding definitions, applied by `at` in the last one, and its body is a chain of `body_calls` zero-argument definitions marked `recursive`.
- `v2_applied_lazy_function_keeps_the_forwarded_depth_of_its_snapshot`: checks (70 passes, 10 body calls) and (63, 10) with max_depth 0, max_states 1, max_work 50000 and asserts `status == "bounded"`, `reason == Some("evaluation")`, `work == 2*passes + 3` (143 and 129) in the style of `v2_applied_lazy_function_body_counts_the_depth_of_its_application`.

Derivation (from the plan's model, not from running after)
Model: the depth `descend` compares is total = evaluation_depth + forwarded; every non-leaf node costs one evaluation_depth; every forwarded hop costs one `consume(1)` and one `forwarded`, not an evaluation_depth; the 128 limit is judged against the total once the context is recursive; a snapshot (lazy function) stores both numbers and resumes from their sum; each call costs one `consume(1)`.
Let P = passes. Invariant is `call(0)`.
1. Calls def0 .. def_P: P + 1 nested `Call` nodes, one evaluation_depth each, so the `at` of def_P is evaluated at evaluation_depth P + 1.
2. `at` descends (P + 2); its target is a `Parameter`, evaluated by `eval_function`, which descends (P + 3). The terminal is reached through P bindings (def_P ... def_1); the P - 1 hops between them are forwarded, so forwarded = P - 1 (the plan's "69" for P = 70).
3. The `Function` node descends once more (P + 4); `Snapshot::new` stores evaluation_depth = P + 4, forwarded = P - 1; total = 2P + 3 (143 for P = 70).
4. Applying the function enters def_{P+1}, a recursive definition, from the snapshot at total 2P + 3. For P >= 63 that is >= 128, so the recursive limit refuses at the first recursive entry: `evaluation-budget`, published as `bounded` / `evaluation`. Work at that point: (P + 1) call charges (def0 .. def_P) + P binding charges + 1 for the application of the function body call = 2P + 2, plus 1 for the select of the key = 2P + 3 (143, 129). The 63 input is the smallest P at which the refusal fires (2*62 + 3 = 127 < 128).
5. If the snapshot lost forwarded (F5c) or hops stopped counting (F1), total is P + 4 = 74 (67 for P = 63), the 10 recursive calls stay under 128 and the run completes with the whole body charged (observed 308 and 280).
Why the input is the same on before: before counts every hop as an evaluation_depth, so its depth at the snapshot is the same total 2P + 3 and the recursive 128 fires at the same place; its 256 limit is never reached on this input (maximum depth 143 + 10), so no 256 refusal moves the result. This satisfies "controls pass on before and after".

Cross-check with the oracle builds (README of the audit: oracle = before + oracle-0120.patch, oracle-after = after + oracle-0120-after.patch, limit 4,096, hits256 = 0 on this input): the control passes on the rtlc lib of both patched trees (logs below), i.e. the expectation agrees with both observation copies.

Results (cargo test --lib, filter = the new test; private target dir per tree; scratch trees hold only crates/rtlc, faults applied to scratch copies, never to the worktree)
| tree | result | log |
| --- | --- | --- |
| after (worktree) | pass | evals/runs/0120-audit/logs/correction-0003-after.log |
| before (before-tree crates/rtlc + the new test file) | pass | evals/runs/0120-audit/logs/correction-0003-before.log |
| after + F5c (scratch copy) | FAIL on values: 70: expected bounded/evaluation work=143, observed complete/None work=308; 63: expected work=129, observed complete work=280 | evals/runs/0120-audit/logs/correction-0003-F5c.log |
| after + F1 (scratch copy) | FAIL on the same values (308, 280) | evals/runs/0120-audit/logs/correction-0003-F1.log |
| oracle (before + oracle-0120.patch) | pass | evals/runs/0120-audit/logs/correction-0003-oracle.log |
| oracle-after (after + oracle-0120-after.patch) | pass | evals/runs/0120-audit/logs/correction-0003-oracle-after.log |
The log copies in the audit record have host paths replaced by placeholders and are cut to the last 60 lines when longer than 80 (as the README says). Raw logs: target/0120-stage-t/c3/log-*.txt.

Lib tests on the after tree: debug 165 passed (log-lib-debug), release 165 passed (log-lib-release); 164 before plus the new test.

Frozen record
- target/0120-stage-t/frozen.sha256: entry for crates/rtlc/src/safety_definition_tests.rs 1c94697e... -> 10cf5876...; previous file kept as frozen.sha256.before-correction-0003. The other 14 entries still match (shasum -c).
- Evaluation record updated: faults/summary.md and the fault table of evals/iterations/0120-forwarding-depth.md (F5c row), corrections/ copies, SHA256SUMS. No product source and no other test changed.

Erratum (added by correction-0004; the text above is kept as written)
- Step 4 and the sentence "The 63 input is the smallest P at which the refusal fires (2*62 + 3 = 127 < 128)" are wrong. The snapshot total T = 2P + 3 is right (127 at P = 62), but from the snapshot the function body adds two levels for `select` (binding check and body entry) and one for the first `Call`, so the first recursive body is judged at 2P + 6. It is refused at 128 or more, which is P >= 61, not P >= 63 (at P = 62 the value is 130).
- With 10 body calls the chain keeps climbing one level per call, so smaller P are also refused later in the chain (P = 57 .. 60, all at work 125). The pair 70 and 63 detects F5c and F1 but not a Snapshot that stores `forwarded` one less (F5d). Correction-0004 replaces the values.
