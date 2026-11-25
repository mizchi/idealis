# 表の構文の評価ループ（plan 0109）

書き方（候補）を、差し替えられる評価の軸で測り続けるためのループ。[plan](../../plans/0109-surface-eval.md)・[記録](../../iterations/0109-surface-eval.md)。

## 回し方

```sh
export ISABELLE=~/opt/isabelle2026/bin/isabelle   # 無ければそのcandidateはunavailable
export IDRIS2_PREFIX=...                          # brewのidris2のprelude
export VEIL_HOME=...                              # lake buildしたVeilのcheckout
just surface-eval-prepare evals/plans/<plan>.json evals/runs/<run>
# agentの軸: evals/runs/<run>/human/<candidate>.md を新しい読み手に渡し、
# 答えのJSONを evals/runs/<run>/human/answers/<candidate>-<n>.json に保存する
just surface-eval-score evals/plans/<plan>.json evals/runs/<run>
```

- `prepare`は既存のrunを拒む（上書きしない）。manifest（plan・入力のsha256・revision・dirtyなpath）を書き、各候補の検査を1度だけ実行して`checks.json`に保存し、automaticとrubricの軸を`results/`に書き、agentの軸の問いを書く。
- `score`は保存した結果と答えから`scorecard.json`・`scorecard.md`を書く。何度やり直しても同じ成績表になる。
- 道具が無い候補は`unavailable`で、合格に数えない（その軸は—）。

## 軸を足す・入れ替える

軸は[axes/](axes/)のmoduleで、[types.ts](types.ts)の`Axis`を満たす: `id`・`title`・`question`・`kind`（`automatic`・`rubric`・`agent`・`mixed`）・`scoring`（得点の付け方の文）・`measure`（必要なら`prepare`）。[axes/index.ts](axes/index.ts)に登録し、planの`axes`に名前を書く。planから外した軸は成績表から消え、他の軸の値は変わらない（0109で確認）。

| 軸        | kind      | 測るもの                                                                                                                    |
| --------- | --------- | --------------------------------------------------------------------------------------------------------------------------- |
| `ddd`     | rubric    | DDDの概念7つ（entityの同一性、設定と前提、値、payload付きの直和、型で排除する状態、遷移の規則、任意の関連）を、0107の証拠で |
| `formal`  | mixed     | 核のIRへ写して参照評価器の判定がrmodelと一致した題材の割合と、答えを出せた検査の種類                                        |
| `machine` | mixed     | 解析できる木を残すか（rubric）と、toolchainの検査が今通るか                                                                 |
| `human`   | agent     | 構文の説明なしで、新しい読み手が振る舞いの問いに答えた正答率（本物の人間ではない）                                          |
| `cost`    | automatic | 空行とコメントを除いた行数（最小との比）、検査の時間                                                                        |

候補は[candidates.json](candidates.json)（ファイル、コメントの書き方、検査command、adapterの出力、rubricの証拠）、題材と人間の軸の問いと答えは[tasks.json](tasks.json)。問いと答えは、結果を見る前にplanと一緒にcommitする。
