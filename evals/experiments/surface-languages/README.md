# モデルの書き方の比較（plan 0107）

同じ5つの題材（[SPEC.md](SPEC.md)）を7つの書き方で書いた。rmodelは筆者（Claude）、他は言語ごとに別の書き手（Claudeのsubagent）が書き、各toolchainで検査した。主要な結果は筆者が手元で再実行した（Quint・Idris2・OCaml・Pkl・Isabelleの一部）。[plan](../../plans/0107-surface-languages.md)・[記録](../../iterations/0107-surface-languages.md)。

| 書き方                        | 検査の方法（実行したもの）                                                                        | SPECとの一致                   |
| ----------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------ |
| [rmodel](rmodel/)             | `rcheck`（参照評価器、scopeの中で完全）                                                           | 書けた性質はすべて一致         |
| [Quint 0.33.1](quint/)        | `typecheck`、`run`（無作為のsimulation、証明ではない）、活性は`verify --backend tlc`（repoのJRE） | すべて一致                     |
| [OCaml 5.5](ocaml/)           | 自作のDSLのBFSとfairなSCCの検査（`run.sh`、警告8をエラー）                                        | すべて一致                     |
| [Isabelle2026/HOL](isabelle/) | `isabelle build`、全性質を証明か反証（`sorry`なし）、nitpickは補助                                | すべて一致（すべての大きさで） |
| [Idris 2 0.8.0](idris/)       | `--check`と、自作のBFSの実行。型の証明は1つ以外は穴                                               | すべて一致                     |
| [Pkl 0.31.1](pkl/)            | `pkl eval`、Pklで書いた型検査器と評価器                                                           | すべて一致                     |
| [Veil（Lean 4）](veil/)       | `#model_check`、`#check_invariants`（cvc5、信頼）、`sat`/`unsat trace`、`#temporal_check`         | すべて一致                     |

## 表現（直接・回避策・書けない）

|                                | rmodel                                  | Quint                         | OCaml                             | Isabelle                       | Idris 2                          | Pkl                                      | Veil                                         |
| ------------------------------ | --------------------------------------- | ----------------------------- | --------------------------------- | ------------------------------ | -------------------------------- | ---------------------------------------- | -------------------------------------------- |
| sortの大きさを検査の引数に     | 直接                                    | 回避（constの集合とinstance） | 直接（大きさの関数）              | 直接（型変数、すべての大きさ） | 直接（暗黙の`n`）                | 直接（check引数）                        | 直接                                         |
| 設定（rel）とfact              | 直接                                    | 回避（initで選ぶ状態変数）    | 直接                              | 直接（引数と仮定）             | 直接                             | 直接                                     | 直接                                         |
| 関係の合成（join）             | 直接                                    | 直接                          | 回避（listの操作）                | 直接（`O`）                    | 回避（`Vect Bool`）              | 回避（joinがなく量化）                   | 直接                                         |
| 数学の整数                     | 直接（scopeの範囲）                     | 直接                          | 回避（機械の整数、guardで範囲内） | 直接                           | 直接（引数は別のsort）           | 直接                                     | 直接（model checkは引数を別のsortに）        |
| payload付きの直和と全域のmatch | 回避（enumと`lone`の関係、`iff`もない） | 直接                          | 直接（網羅性はcompilerの警告）    | 直接（guardは`fun`に）         | 直接                             | 直接（網羅性は自作の検査、wildcardなし） | 回避（SMTでは`match`が使えず、構成子の等号） |
| ShippedCancelledを型で排除     | 書けない（checkで到達不能）             | 型で排除                      | 型で排除                          | 書けるが空（証明）             | 型で排除（証明）                 | 状態としては書けない                     | 型で排除（定理）                             |
| 二状態の性質（ShippedStays）   | 書けない                                | 回避（履歴の変数）            | 直接                              | 直接                           | 直接                             | 直接（`next`）                           | 直接（`□`と`◯`）                             |
| 数え上げ（`#active < 2`）      | 直接                                    | 直接                          | 直接                              | 回避（`finite`と`card`）       | 直接                             | 直接                                     | 回避（異なる2人がいない）                    |
| `lone`                         | 直接                                    | 直和型                        | `option`                          | `option`                       | `Maybe`                          | 直接                                     | 回避（`Option`がSMTで壊れ、関係と不変条件）  |
| 活性とweak fairness            | 書けない                                | 直接（TLCで検査）             | DSLに実装（SCC）                  | 直接（traceの上で証明）        | 型で書けるが証明は穴、実行で検査 | 直接（データ、評価器はleads-toの形だけ） | 直接（stutterのactionが要る）                |

## 大きさ（空行とコメントを除く行数の概算）

|                        | permissions | bank | order | sessions | service |  計 |                      共有の部分 |
| ---------------------- | ----------: | ---: | ----: | -------: | ------: | --: | ------------------------------: |
| rmodel                 |          24 |   17 |    35 |       31 |      17 | 124 |                               0 |
| Quint                  |          61 |   27 |    58 |       63 |      34 | 243 |                               0 |
| OCaml                  |          42 |   22 |    42 |       34 |      26 | 166 |              170（DSLと検査器） |
| Isabelle（証明を含む） |          51 |   25 |    42 |       73 |      65 | 256 |                              38 |
| Idris 2                |          79 |   60 |   108 |       75 |      72 | 394 |              299（DSLと検査器） |
| Pkl                    |          42 |   19 |    51 |       31 |      23 | 166 | 410（schema・型検査器・評価器） |
| Veil                   |          43 |   31 |    53 |       49 |      34 | 210 |                               0 |

## 書き手が報告した詰まりどころ

- **Veil**: Leanの`def`と`match`をguardに使うと、SMTの経路では中身が見えず、guardが禁じる遷移を含むtraceを返した（書き手の報告、筆者は未再現）。`Option`もSMTの写しで壊れる。明示状態の検査は正しい。
- **Isabelle**: nitpickは定義を畳んだままだと反例を見逃し（sessionsの`LockedIsActive`）、`card`の周りで偽の反例を出した。証明は通る。
- **Idris 2**: derivingがなく、`Eq`と有限性のinstanceを手で書く（モデルごとに10〜20行）。brewの`idris2`は`IDRIS2_PREFIX`の設定が要る。
- **OCaml**: sortの要素がただの`int`なので、UserとDocの取り違えを型が捕まえない。fairnessをactionの名前の文字列で指す。
- **Pkl**: amendしたmoduleの`new`の中では関数が見えず、builderの関数で組む。formulaは型付きのASTで、JSONをIRへ文字列の解析なしで写せる。
- **Quint**: sortに大きさを付けられず、設定を状態変数で表す。`run`はsimulation。
