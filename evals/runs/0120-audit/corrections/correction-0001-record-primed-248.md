Correction 0001 to the stage T freeze: record-hop and primed-hop accepted size 249 -> 248

Trigger
- Stage I measured the depth before each check (target/0120-stage-i/07-scratch-peaks.log, 08-scratch-peaks-before.log).
- Before the change, record-hop and primed-hop have depth 2N+7 (N=124 gives 255). After the change it is N+7: N=248 peaks at 255 and N=249 at 256 (refused).
- The frozen rows used 2N+6 / N+6, so the after boundary was extrapolated one step too far (accepted 249, first refused 250).

Why the stage T constant was wrong
- The constant was fixed with a probe that added one wrapper level around step and inv and compared the last accepted N.
- For the invariant-path families (set-hop, function-hop, let-hop) the extra level shifts the depth, so the parity was resolved correctly. set-hop is really 2N+6 and its frozen rows are unchanged.
- For record-hop and primed-hop the deepest path is inside the Next assignment. The probe's last accepted N did not move (124 with and without the extra level), which is only consistent with 2N+7 if the action-level wrapper adds nothing to the evaluation depth. The parity was therefore resolved to the wrong value. This explanation is inferred from the data, not separately measured.
- The pre-change boundary (b = 124) and the work formulas (19N+53, 20N+47) were not affected.

Changed rows (record-hop, primed-hop only)
- accepted rows: [10, 124, 249] -> [10, 124, 248]; work 19*248+53 = 4765 and 20*248+47 = 5007.
- first refused row: 250 -> 249 (`error protocol: Safety evaluation nesting exceeds 256`).
- metadata depth / formerDepth: 7+N / 7+2N. 124 -> 131 / 255, 248 -> 255 / 503, 249 -> 256 / 505. The N=10 rows change to 17 / 27 for the same reason (structural values; a recorded peak below depth 128 is one lower).

Changed files
- crates/rtla-compiler/tests/call_depth.rs (4 lines: two accepted lists, two refused rows)
- evals/experiments/call-depth/sources-0120.ts (levels and depth formulas of the two families)
- fixtures/call-depth/cases-0120.json, records-0120.json (regenerated with baseline-0120.js + expect-0120.js from the sources; previous copies in target/0120-stage-t/before-correction/)
- target/0120-stage-t/frozen.sha256 (4 entries rewritten; previous file in frozen.sha256.before-correction). The other 11 entries still matched before the rewrite.

Evidence
- 31, 32: pre-change binaries built from `git archive HEAD` (b57c0a376) into target/0120-stage-t/before-tree (no change to the worktree or to git).
- baseline-0120 on the pre-change binaries reproduces every unchanged record exactly; the only differences are record-hop/primed-hop 248 (new) and 250 (removed). 248 is refused on the pre-change code for both frontends.
- 33: corrected call_depth.rs on the pre-change code: record-hop 248 and primed-hop 248 are refused (Red reason); the 249 refusal row passes there too.
- 34, 35: corrected call_depth.rs on the current tree 5 passed; contract-test 68 passed.
- 36: vp check pass; cargo fmt --check (rtla-compiler) pass.
No product code was touched by this correction.
