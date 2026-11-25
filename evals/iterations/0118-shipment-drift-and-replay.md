# 0118 発送の仕様と3言語の再生

自然言語の規則 `SHIPMENT-TRACKING` から `ShippedStays` と、発送済みの状態に到達する `WasShipped` を参照する。用語とリンクは Pkl に置き、Markdown と意味比較の manifest を生成する。rspec はリンク名と判定の種類を検査し、rmodel の参照評価器が規則の成立と発送済みへの到達を確認する。 [計画](../plans/0118-shipment-drift-and-replay.md) · [要件](../../spec/rmodel-design.md#shipment-tracking) · [モデル](../../fixtures/rcheck/structured/shipped-transition.model)

| 対照                                                           | 結果                                                                                                                                                 |
| -------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| リンク名だけを RenamedShipmentCheck に変更                     | rspec が参照先の欠落と ShippedStays の未参照を報告した。ShippedStays に reachable を指定した対照も拒否した。                                         |
| 自然言語だけを同義に言い換える、または追跡方法の変更を許可する | Jev の準備結果で、正常文と2対照の対象抜粋が異なり、参照モデルが同一であることを検査した。TypeSafe の資格情報が利用できず、ライブの意味判定は未計測。 |
| モデルの確認操作で追跡方法を Post に上書き                     | rmodel の参照評価器は2操作目で ShippedStays の違反を報告し、同じ入力列でのトレース生成も確認操作を拒否した。                                         |
| 各言語の実装だけで同じ上書き                                   | Rust・TypeScript・Go は正常時に両トレースの全状態で一致した。上書き版は Courier の2操作目で初めてずれ、Post は一致した。                             |

同じ `ship(Courier) → acknowledge` において、変更を許す対照文は確認後の Post を許容する。現行規則は Courier の維持を要求し、モデルもその変更を拒否する。比較対象はこの一歩の許容範囲である。

Courier と Post の ITF は、指定した初期状態を検証し、各操作後の期待値を rmodel の `validate` から得る。三言語のテストは生成したインターフェースを実装し、初期状態から順に同じ ITF と照合する。TypeScript の fast-check と Go の fuzz seed に固定例を含めた。この再生で確認した範囲は、各追跡方法での発送と直後の確認である。 [Courier の ITF](../../fixtures/rcheck/shipment/courier.itf.json) · [Post の ITF](../../fixtures/rcheck/shipment/post.itf.json)

- `just shipment-test` は静的リンク、モデルと生成物、3言語の再生をまとめて検査する。
- `just shipment-write` は ITF とインターフェースを再生成する。
- `just shipment-drift .artifacts/shipment-drift` は新規ディレクトリに比較の入力と判定を保存する。資格情報がなければ入力を準備し、未計測として終了する。
