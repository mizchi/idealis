# 同じ保存部品を二つのストアにつなぐ

同じ要求 ID が東と西に届いても、ストアごとに別の版を保存し、配信は接続先の版を返す。東西のストアと配信を、それぞれ一つのファイルから二度構成する。[接続定義](model.project.json) · [保存](storage.ideal) · [配信](delivery.ideal)

| インスタンス   | 自分の状態              | 共有口 `@receipt`        |
| -------------- | ----------------------- | ------------------------ |
| `east`         | `east_head`             | `eastReceipt` を読み書き |
| `west`         | `west_head`             | `westReceipt` を読み書き |
| `eastDelivery` | `eastDelivery_received` | `eastReceipt` を読む     |
| `westDelivery` | `westDelivery_received` | `westReceipt` を読む     |

`@commit` は接続後の操作列に `east_commit`、`west_commit` として現れる。二つの受領記録と検査する性質はプロジェクト側に宣言し、要求 ID の型も共有する。[契約](contract.ideal)

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml --bin idealis-check
checker=crates/idealis-check/target/debug/idealis-check
model=examples/model-patterns/offline-merge/two-stores/model.project.json
"$checker" component examples/model-patterns/offline-merge/two-stores/storage.component.json
"$checker" component examples/model-patterns/offline-merge/two-stores/delivery.component.json
"$checker" witness "$model" IndependentReceipts
"$checker" "$model" --checker all --question westDelivery_DeliveredFromReceipt
"$checker" generate "$model" go
"$checker" generate "$model" go --public
```

manifest の `exports` は `east_commit` と `west_commit` を選ぶ。公開用の Go interface はこの二つの操作を持ち、公開用のデコーダーは配信コマンドを拒否する。完全モデルの照合には通常の生成物と、配信を含む証拠経路を使う。

保存と配信は同じ[受領記録の型](receipt-interface.ideal)と[要求 ID の型](request-types.ideal)を使う。保存のポートは書き込み、配信のポートは読み取りを宣言する。部品単体で型と読み書きを検査し、東西への接続で同じ型・多重度の共有変数を要求する。プロジェクトの到達性はリンク後に検査する。

配信の動作契約は、入力の受領記録が一度付いたら変わらないことを仮定し、配信した版が受領記録に一致することを保証する。`CanDeliver` では配信まで実際に進めることも確認する。二つのストアの合成と、配信だけを一つの環境につなぐ対照を検査できる。[配信の宣言](delivery.component.json) · [配信だけの環境](delivery-harness.project.json)

```sh
"$checker" contracts "$model"
"$checker" contracts examples/model-patterns/offline-merge/two-stores/delivery-harness.project.json
```

配信だけの環境では `publish → subscriber_send` の2手を取得する。環境から `publish` を取り除くと仮定と保証は成立するが `CanDeliver` が到達不能になる。保存部品に受領記録を消す操作を加えた対照はポートの型が合い、`commit → forget` の2手で配信側の仮定を破る。契約の検査は、それぞれ指定した環境と有限範囲に対する結果を返す。[契約の対照](../../../../crates/idealis-check/tests/components.rs)

証拠経路の5手で、要求 `First` は東で版2、西で版1を得た。独立した SQLite の読み書きで照合し、西の配信が東の記録を読む故障は5手目で期待1・実測2になった。プロジェクト側の照合では、配信による代入、別インスタンスの状態への代入、同じ共有状態への二重の書き込み権限を拒否した。西の配信で `@receipt` を `eastReceipt` に置き換える対照も、配信操作の宣言位置で読み取りを拒否する。[SQLite の照合](../../../../tests/model-patterns/two-stores.test.ts) · [権限の回帰試験](../../../../crates/idealis-check/tests/linked_projects.rs)
