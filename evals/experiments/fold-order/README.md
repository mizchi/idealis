# 集合 fold の順序の実験

0089 の測定と採取に使った駆動コード。実行結果は `evals/runs/0089*` に保存してあり、ここのコードを変えても保存済みの run は書き換えない。再実行は新しい run ID で行う。

| ファイル       | 内容                                                                                                                                   | 再現                                                                                                                                                                                                 |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `grid.rs`      | 14 の集合 × 6 の body を v1・v2 の両 IR で `check` し、結果の全文字列（statusとwork、または拒否の文言）を比べて 1 行ずつ出力する       | `crates/rtla-compiler/examples/` へ複製し、`cargo run --locked --release --example <名前> > <run>/grid-after.tsv`。実行後に複製を消す                                                                |
| `fold_cost.rs` | 64 要素の集合を累積値への `\union` で fold する Next の `check` の work と中央値時間を、整数と record・状態数 1 と 64・v1 と v2 で測る | `crates/rtlc/examples/` へ複製し、`cargo run --locked --release --example fold_cost` を 3 回。実行後に複製を消す                                                                                     |
| `capture.mjs`  | `foldIndependentControls` の入力を固定 TLC で採取し、`fixtures/tlc-apalache-folds/` へログと `expected.json` を書く                    | `just build` の後、リポジトリの root で `TLC_JAVA`・`TLC_JAR`・`TLC_APALACHE`（pinned の sha256 を検証する）を設定して `node evals/experiments/fold-order/capture.mjs`。採取済みの名前があると止まる |

`0089-paths`・`0089-new-path`・`0089-tlc` の結果と `0089-audit/commands.tsv` は、駆動コードがこの場所へ移る前の記録で、当時のパス（`evals/runs/0089-*/`）のまま残してある。それらの run は変更前の木の測定で、後続の `0089b-*`・`0089c-*`・`0089d-*` が修正後の木を測る。
