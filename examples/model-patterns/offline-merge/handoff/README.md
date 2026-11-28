# 保存した受領記録から応答する

編集を保存する処理と、HTTP 応答を配信する処理を分ける。二つの要求 ID と、その要求を保存した版を扱う。保存の完了前に接続を開けるが、その要求の受領記録ができるまで応答は待つ。接続を開き直した場合も、同じ要求 ID の版を返す。[共有する受領記録](contract.model)

| 断片 | 担当する状態                                           | 操作                                                                |
| ---- | ------------------------------------------------------ | ------------------------------------------------------------------- |
| 保存 | 最新版 `head` と、要求ごとに固定する `receipt`         | `commit` が最新版と受領記録を一緒に更新する                         |
| 配信 | 待っている接続 `waiting` と、実際に返した版 `received` | `open` と `drop` は接続を変え、`send` は同じ要求の `receipt` を読む |

保存だけのプロジェクトで、受領記録の版が最新版の範囲にあり、別の要求の保存が以前の受領記録を変えないことを検査できる。合成したプロジェクトでは、配信した版が同じ要求の受領記録と一致することも検査する。ファイル間では一つの名前空間を使う。書き手と読み手の分担はモデルの操作と遷移条件に記述している。[保存](storage.model) · [配信](delivery.model)

```sh
cargo build --locked --manifest-path crates/rcheck/Cargo.toml
rcheck=crates/rcheck/target/debug/rcheck
pattern=examples/model-patterns/offline-merge/handoff

"$rcheck" "$pattern/storage.rmodel.json" --checker reference
"$rcheck" "$pattern/project.rmodel.json" --checker all --question DeliveredFromReceipt
"$rcheck" witness "$pattern/project.rmodel.json" WaitingForFirstReceipt
just build
node --test dist/tests/model-patterns/edit-handoff.test.js
```

実行試験は SQLite の受領記録と HTTP で受け取った版を照合する。先に接続を開く経路と、先に保存してから接続する経路を試した。版1を保存した直後に接続を切り、DB を開き直し、別の要求を版2で保存してから最初の要求へ応答しても、返る版は1だった。保存前に応答する故障は1手目、要求ごとの版を最新版にすり替える故障は6手目で検出した。[実行試験](../../../../tests/model-patterns/edit-handoff.test.ts)

保存と配信をストアごとに繰り返す例では、名前空間と共有変数の書き込み権限も接続時に指定する。[同じ部品で東西をつなぐ](../two-stores/README.md)
