# 0119-cli-r2

Corrected measurement of the CLI Wasm (wasm32-wasip1, `node:wasi`) for the `operator` and `hop` cells of iteration 0119. `../0119-wasm-ceiling/cli-after.json` and `cli-oracle.json` stay as recorded. Their `operator` row (`largest: 7`, `Invariant Inv is violated` at size 8) is a model violation, not a nesting limit: for even sizes `negN(counter)` equals `counter`, and the invariant was `<= 0`, so it fails once `counter` reaches 1. The invariant in `cli-ceiling.ts` is now `<= 4`, which holds for every `counter` in 0..4 at every size.

Host paths are replaced by the placeholders of `../0119-audit/README.md` (`<scratch>`, `<after>`, `<home>`, `<user>`). Every command ran through `../0119-audit/drivers/run.sh` and has one row in `commands.tsv` (start, end, load, exit) and one log in `logs/`.

| What | Value |
| --- | --- |
| Tree | HEAD 61468f8bc plus the one-line `Inv` change in `evals/experiments/call-depth/cli-ceiling.ts` (`checks.txt`) |
| Build | `just build` (TypeScript to `dist/`), then the CLI Wasm rebuilt with the command of `0119-audit/drivers/slot-f.sh`: `crates/rtlc/src/safety_expressions.rs` changed in 61468f8bc after the earlier Wasm was built |
| Search | `hop:1:300 operator:1:300`, the ranges of the `after` run in `slot-f.sh` |
| Not measured | The `oracle` side of `slot-f.sh` (`hop:1:1400 operator:1:2500`); its `operator` row in `0119-wasm-ceiling/cli-oracle.json` is still the violation |

| cell | largest accepted | first refused | refusal | before (`0119-wasm-ceiling/cli-after.json`) |
| --- | ---: | ---: | --- | --- |
| hop | 16 | 17 | `exit 150: Unsupported TLA input: expression or operator nesting exceeds 40` | 16 / 17, same refusal |
| operator | 17 | 18 | `exit 150: Unsupported TLA input: expression or operator nesting exceeds 40` | 7 / 8, `exit 12: Error: Invariant Inv is violated.` |

Both cells end at the frontend refusal of the v1 IR (nesting 40), well below the v2 limit of 256. The CLI has no IR version option, so it cannot reach the v2 limit.
