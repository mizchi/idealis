Correction 0004 to the stage T freeze: boundary values of the Snapshot control (fault F5d, and the derivation error of correction-0003)

Trigger
- The final review found that correction-0003's boundary derivation is wrong (see the erratum in correction-0003) and that the control misses a Snapshot that stores `forwarded` one less (F5d, `context.forwarded.saturating_sub(1)`): the values 70 and 63 pass under it.
- Classification: Evidence. The design stands; only the control's input values change.

Change (crates/rtlc/src/safety_definition_tests.rs only, the one test `v2_applied_lazy_function_keeps_the_forwarded_depth_of_its_snapshot`; the helper, every other test and all product source are untouched)
- The refused list grows from (70, 63) to passes 70, 63, 62, 61, 60, 57 with 10 body calls; one completing value, passes 56, is added (status and reason only, no work).

Derivation (from the plan's model: every non-leaf node costs one `evaluation_depth`, every forwarded hop one `forwarded`, the recursive 128 judges the total once the context is recursive and refuses a judged value of 128 or more, the judged value is the depth before the descent, a leaf judges only at depth 128 or more, `select` descends twice, each call and each hop costs one `consume(1)`)
P = passes, k = body calls (10), T = snapshot total.
1. Calls def0 .. def_P are P + 1 `Call` nodes; the `at` of def_P is entered at P + 1, its target `Parameter` at P + 2, the `Function` after P - 1 further hops at 2P + 2. `Snapshot::new` stores total T = 2P + 3.
2. Applying the function: `select` judges T and T + 1 (two descents), the `Call` of def_{P+1} judges T + 2 (context not yet recursive, only 256 applies) and enters a recursive definition. Inside, the nested calls judge T + 3, T + 4, ..., the call of def_{P+k} judges T + k + 1, and the leaf body `TRUE` is at depth T + k + 2 and judges only if that is 128 or more.
3. First recursive judgement is 2P + 6: refused at once for P >= 61 (P = 62: 130; P = 61: 128). Work before that refusal: (P + 1) calls + P hop charges + 1 key select + 1 for the call of def_{P+1} = 2P + 3. P = 70: 143, 63: 129, 62: 127, 61: 125.
4. For P <= 60 the chain delays the refusal: the judged value reaches 128 at the nested call number j = 128 - (2P + 6) + 1; the calls before it are charged, so work = 2P + 3 + (128 - 2P - 6) = 125. This holds while the refusal falls inside the chain, 2P + k + 4 >= 128, i.e. P >= 57 for k = 10: P = 60 and P = 57 give 125 (P = 57 is the smallest refused passes for 10 body calls).
5. P = 56: the deepest call judges 2P + k + 4 = 126 and the leaf is at depth 127 (no leaf judgement), so the run completes: status `complete`, reason None. P = 57 is the smallest refused value, so (56, 57) bracket the model's boundary.
   Work of the complete run is NOT asserted. The charges of the refused prefix are derived (calls P + 1, hops P, the key select 1, the call into def_{P+1} 1; the nested calls k - 1 more, so one invariant evaluation costs 2P + k + 2 if leaves are free). The rest cannot be derived from the plan: the plan does not say whether a leaf (the key literal, `TRUE`) or an `assign` step is charged, nor how many times the invariant is evaluated for the 1 state / 2 generated shape of `program()` (its hop figures, 10N + 32 for 5 / 6 and 2N + 8 for 1 / 2, fit two evaluations per distinct state, not per generated state, and the Next here does not use the chain). The observed 4P + 28 (308 at 70, 280 at 63 under F5c; 252 at 56 after) fits 2 * (2P + k + 2) + 4 but that is a fit to observations, not a derivation, so it is kept in the log only (target/0120-audit-raw/logs/log-*.txt) and the earlier assertion of 252 was removed.
6. Fault values: F5c (forwarded 0) and F1 (hops do not count): total P + 4, nothing refused, complete (observed work 308, 280, 276, 272, 268, 256), the control fails on every refused value. F5d (T - 1): P = 62 and above still refused at the first judgement with the same work, but P = 61, 60 and 57 refuse one call later, work 126 instead of 125.

Why before gives the same values: before counts every hop as an `evaluation_depth`, so its depth at the snapshot is the same total 2P + 3, the recursive 128 fires at the same judgement and its 256 limit is never reached on these inputs (maximum 2P + k + 6 well under 256).

Results (cargo test --lib, filter = the test; one private target directory per tree, scratch trees hold only crates/rtlc, faults applied to scratch copies, never to the worktree)
| tree | result |
| --- | --- |
| after (worktree) | pass |
| before (before-tree crates/rtlc + the new test file) | pass |
| oracle (before + oracle-0120.patch) | pass |
| oracle-after (after + oracle-0120-after.patch) | pass |
| after + F5c (forwarded: 0) | FAIL on all six refused values: observed complete, work 308, 280, 276, 272, 268, 256 |
| after + F1 (hops do not count) | FAIL on the same six values and the same works |
| after + F5d (forwarded.saturating_sub(1)) | FAIL on 61, 60, 57: expected work 125, observed bounded/evaluation work 126 (70, 63, 62 and the completing 56 pass) |
rtlc lib: debug 165 passed, release 165 passed.

Frozen record
- target/0120-stage-t/frozen.sha256: entry for crates/rtlc/src/safety_definition_tests.rs 10cf5876... -> 42ee8431... -> 36ea8059...; previous file kept as frozen.sha256.before-correction-0004-r2 (the first 0004 version asserted work 252 for passes 56; frozen.sha256.before-correction-0004 is the file before 0004). The other 14 entries still match (shasum -c, 15 OK).
- Evaluation record updated: faults/summary.md (F5c row, new F5d row), the fault table and verdict line of evals/iterations/0120-forwarding-depth.md, corrections/ copies. No product source and no other test changed.
