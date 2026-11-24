# 0098: 文法の候補を、意味と結果が読み取れるかで評価するループ

[plan](../plans/0098-syntax-readability.md)のとおり、核のモデルをQuint風のテキストに出力するprinter（`rmodel::quint_like`、v0）と、参照評価器から問いと正解を作る仕組み（`examples/readability.rs`）を作った。dev（seed 300〜の空でない10モデル、42問）を、文脈を持たないhaikuのsubagent 2体に、文法の説明なしで読ませた。

## v0の結果

| 問い                 |  正答 |
| -------------------- | ----: |
| invariantの判定      |  9/10 |
| reachableの判定      | 10/10 |
| actionを実行できるか | 17/22 |
| 計                   | 36/42 |

[prompt](../runs/0098-readability-v0-development/)・[正解](../runs/0098-readability-v0-development/key.json)・[答え](../runs/0098-readability-v0-development/answers-0.json)（[1](../runs/0098-readability-v0-development/answers-1.json)）・[採点](../runs/0098-readability-v0-development/score.json)。読み手2体で約11万token。

## 誤答の分類

- **sortの合わない代入（5件）**: 306の`a0`・`a1`、307の`a2`、310の`a1`・`a2`。どれも`v1: Set[S0]`へenumの値や別のsortの集合を代入するactionで、今の意味論（型を破る遷移は起きない）では実行できない。読み手は5件とも、条件がない・成り立つので実行できると答えた。構文から、この遷移が起きないことは読み取れない。
- **sortの合わない像（1件）**: 302の`Inv`（`v1.preimage(v1.image(S1))`、`v1: E -> Set[S1]`）。keyのsortが違うので像は常に空で、invariantは成り立つ。読み手は破れると答えた。imageとpreimageの向きの読み違いか推論の誤りかは区別できない。

6件すべてが、sortの合わない式を含む。文法の見た目ではなく、sortの合わない式を許すrmodelの型検査（arityだけを見る）から来ている。

## 分かったこと

- 誤答は、0097の後で決めた意味論v0（型・多重度の違反を報告する）を支える。黙って起きない遷移は、読み手に読み取れない。
- 次の仮説: sortの合わない式は、静的な型検査で拒否する。生成器も、sortの合ったモデルだけを作るようにする。
- 生成したモデルは式が意味のない組み合わせで、問いが構文の読み取りより式の計算を測りやすい。候補どうしの比較には使えるが、絶対値の解釈には注意が要る。

確認用の問い（seed 2000〜）は、まだ使っていない。
