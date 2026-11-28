# 0097: rmodelの参照評価器と、写し方の差分テスト

[plan](../plans/0097-rmodel-reference.md)のとおり、rmodelの意味論の正本として参照評価器（`crates/idealis-model/src/reference.rs`）を作り、型の合ったモデルを乱数で生成して、参照評価器・rtlc（TLA+の写し方）・ralloyのSAT（Alloyの写し方）を比べた。意味論は[docs/kernel-semantics.md](../../docs/kernel-semantics.md)に、文法から独立してまとめた。

- **corpus**: 参照評価器の判定は、仕様のclaim 32件（0091・0093・0094）すべてと一致した。
- **生成したモデル**: dev（seed 0〜299）・確認（seed 1000〜1299）それぞれ300モデル・600 checkで、判定が出たものの間に食い違いは0件。
- **rtlcのバグ**: rtlcは、TLC 1.8.0が受け付けるTLA+に対してエラーを返した（[#112](https://github.com/mizchi/idealis/issues/112)）。2つ目の定義域にフィルタの内包表記を持つ、束縛が2つの集合の内包表記で`Invalid bound variable slot`（dev 32件・確認44件）。`Invalid safety variable read`（dev 14件・確認12件）はまだ最小化していない。

## 結果（check数）

| 分類                                                            | dev | 確認 |
| --------------------------------------------------------------- | --: | ---: |
| 3つのengineが一致（成り立つ・破れる・到達できる・到達できない） | 359 |  324 |
| 空（初期状態がない）で一致                                      | 180 |  190 |
| rtlcのエラー                                                    |  46 |   56 |
| rtlcの予算切れ（200,000状態）                                   |  17 |   33 |
| 参照評価器の予算切れ（SATは成り立つ）                           |   4 |    1 |
| 判定の食い違い                                                  |   0 |    0 |

[dev](../runs/0097-rmodel-reference-development/results.jsonl)・[確認](../runs/0097-rmodel-reference-confirmation/results.jsonl)。エラーと予算切れのモデルは各runの`models/`にある。`cargo run --release --example differential -- <first> <last> <output>`で再現できる。

## 計画からの差

- **空**: planの意味論は、初期状態のないモデルで判定を「成り立つ」「到達できない」（空の範囲で真）としていた。スモーク（seed 5000〜）で、rtlcは`empty`、SATはno-instanceと答え、用語集も`empty`を合格ではないと定義していた。参照評価器に「空」を加え、SATのno-instanceと対応させた（checkのno-instanceは空、runのno-instanceは空か到達できない）。比較の規則は緩めていない。dev・確認とも、この変更の後に初めて走らせた。
- **文法**: 生成したモデルで、`(A + B) in C`のように括弧で始まる集合式を、rmodelのparserが論理式の括弧として読んで拒否した。括弧の中を論理式として読めないときは、集合式として読み直すようにした。
- **生成器**: `none`をjoinの項に置くと型検査が拒否するので、joinの項には置かないようにした（スモークで見つけた。dev・確認の前）。
- 生成したモデルの約3割が空だった。initとfactが満たせない組み合わせが多い。

## 意味論の決定（v0）

mizchiと相談して、次を決めた。0097の参照評価器と写し方は、まだ変更前の意味（型・多重度を破る遷移は起きない）で比べている。

- 整数は数学の整数。範囲外は未確定。
- 初期状態がなければ空（合格ではない）。
- 型・多重度を破る遷移は、違反としてwitness付きで報告する。
- 定義の呼び出しは、引数の式を代入する。
- 部分的な演算は、未定義のエラーとしてwitness付きで報告する。

## 次の段階

- 0098: 型・多重度の違反を報告する意味を、参照評価器と2つの写し方に入れる。
- 検査器を、核の問い（invariant・reachableなど）と対応する断片を宣言する共通の枠に載せ、まだない形式化も同じ枠に足せるようにする。
