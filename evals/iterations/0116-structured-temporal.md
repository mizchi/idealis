# 0116 有限列・値付き variant・公平性

[事前条件](../plans/0116-structured-temporal.md)に沿い、列と variant を名前付きの値型として IR に追加した。進行性は状態不変条件や到達可能性から分け、時相の問いごとに公平性を指定できる。既存の IR は新しい項目を省略できる。

| 固定対照                         | 観測                                                                                                                                                                                          |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 長さ2のログ                      | 参照評価器と rtlc で `[A,B]` と `[A,A]` はそれぞれ2遷移で到達し、TLC でも到達した。負の添字・長さ以上の添字は空。上限ガードを外すと3遷移で Scope が破れ、idealis-check の安全性は `unknown`。 |
| 決済結果                         | `Paid(Amount)` と `Rejected(Reason)` の両方に到達し、金額0の承認は到達しない。参照評価器・rtlc・TLC で判定が一致した。異なるペイロード型は `validate` が拒否した。                            |
| 配信と接続の切断                 | 接続が続く配信は弱公平性ありで成立し、公平性なしで待ち続ける反例が出る。接続を切り替えられる配信は弱公平性だけでは反例が出て、配信に強公平性を課すと成立した。rtlc と TLC で一致。            |
| 問いごとに公平性を変えるサービス | Pkl の `EventuallyServed` と `EventuallyServedUnfair` を IR に写した。rmodel の直接記述と Pkl 由来の IR は rtlc で同じ判定になり、直接記述は TLC でも一致した。                               |
| Pkl の注文                       | ペイロード付きの状態と式の条件分岐を IR に写した。既存の状態の問い2件の判定・深さは既存の rmodel と一致し、配送情報を遷移前後で比較する問いは `omitted` に残した。                            |

idealis-check は空の初期状態と予算切れを区別する。rtlc の時相の反例には `lasso.loopStart` を含めた。反例の状態は backend の出力形式。到達確認・実装との照合は状態の遷移系を使い、公平性は進行性の問いにかかる。

Rust・Go・TypeScript の生成コードで順序の付いたスナップショットとペイロードを読み、MoonBit の生成コードは JS ターゲットで型検査した。idealis-check の実装を呼び出すテストでは列の順序を逆にした状態を不一致として検出した。PBT では列の整数要素を1つだけ変える対も生成できる。

再現経路は `just idealis-check-structured`、`just idealis-check-adapters`、`just idealis-model-structured-tlc .artifacts/0116-replay`。TLC の確認ランは17件。配信の公平性は引数の少なくとも一つで実行された操作全体に対して課し、引数ごとの公平性には追加の記述が必要。任意の時相式・二状態の性質・完全な Raft はこの評価の対象外。

[モデル](../../fixtures/rcheck/structured) · [設計判断](../../crates/idealis-model/grammar-decisions.md) · [自然言語の仕様](../../spec/rmodel-design.md#idealis-model-structured)
