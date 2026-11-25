# 成績表（0109-surface-eval）

各軸は0〜1。—は未測定（合格に数えない）。1つの点にまとめない（重みなし）。

| 候補 | DDD記述 | 形式化との相性 | 機械での理解容易性 | 人間への理解容易性（代理） | 道具の費用 |
| --- | ---: | ---: | ---: | ---: | ---: |
| rmodel | 0.64 | 0.88 | 1 | 1 | 1 |
| quint | 0.79 | 0.25 | 1 | 1 | 0.51 |
| ocaml | 0.93 | 0.13 | 0.50 | 1 | 0.75 |
| isabelle | 0.93 | 0.13 | 0.75 | 1 | 0.48 |
| idris | 0.93 | 0.13 | 0.50 | 1 | 0.31 |
| pkl | 0.86 | 0.53 | 1 | 1 | 0.75 |
| veil | 0.79 | 0.38 | 0.75 | 0.94 | 0.59 |

## DDD記述（ddd、rubric）

DDDの概念をそのまま書けるか。得点: 0107の証拠（直接1・回避0.5・書けない0）の、DDDの概念7つの平均。

| 候補 | identity | config | ints | adt | typeExclusion | step | lone |
| --- | --- | --- | --- | --- | --- | --- | --- |
| rmodel | 1 | 1 | 1 | 0.50 | 0 | 0 | 1 |
| quint | 0.50 | 0.50 | 1 | 1 | 1 | 0.50 | 1 |
| ocaml | 1 | 1 | 0.50 | 1 | 1 | 1 | 1 |
| isabelle | 1 | 1 | 1 | 1 | 0.50 | 1 | 1 |
| idris | 1 | 1 | 0.50 | 1 | 1 | 1 | 1 |
| pkl | 1 | 1 | 1 | 1 | 0 | 1 | 1 |
| veil | 1 | 1 | 0.50 | 0.50 | 1 | 1 | 0.50 |

- rubric: the writers' 0107 reports, not measured again
- identity: entity identity with a check-time size
- config: configuration and its premises
- ints: value objects (integers and data)
- adt: algebraic data with payloads and total match
- typeExclusion: impossible states ruled out by types
- step: business rules over transitions
- lone: optional associations

## 形式化との相性（formal、mixed）

核のIRと検査器にどこまで載るか。得点: IRに写せて参照評価器の判定がrmodelと一致した題材の割合（adapterがなければ0）と、答えを出せた検査の種類の割合（4種類）の平均。

| 候補 | adapter | irMatched | irUnsupported | verification | liveness |
| --- | --- | --- | --- | --- | --- |
| rmodel | 1 | 5 | 0 | complete-search bounded inductive | 0 |
| quint | 0 | — | — | bounded complete-search | 1 |
| ocaml | 0 | — | — | complete-search | 0.50 |
| isabelle | 0 | — | — | proof | 1 |
| idris | 0 | — | — | complete-search | 0.50 |
| pkl | 1 | 4 | 1 | complete-search | 0.50 |
| veil | 0 | — | — | complete-search bounded inductive | 0.50 |

- verification kinds: rubric from 0107 (complete-search, bounded, inductive, proof)

## 機械での理解容易性（machine、mixed）

道具がモデルを解析し、検査できるか。得点: 解析できる木（rubric: 1・0.5・0）と、toolchainの検査が今通るか（1・0、使えなければ未測定）の平均。

| 候補 | tree | toolchain |
| --- | --- | --- |
| rmodel | 1 | passed |
| quint | 1 | passed |
| ocaml | 0 | passed |
| isabelle | 0.50 | passed |
| idris | 0 | passed |
| pkl | 1 | passed |
| veil | 0.50 | passed |

- tree: rubric from 0107 (1 an analysable tree, 0.5 analysable only inside the host, 0 host functions)
- toolchain: the candidate's check commands, run in this run

## 人間への理解容易性（代理）（human、agent）

構文の説明なしで、読み手が振る舞いを読み取れるか。得点: 新しい読み手の、振る舞いの問いの正答率（本物の人間ではない）。

| 候補 | correct | questions | promptLines | leaks | wrong |
| --- | --- | --- | --- | --- | --- |
| rmodel | 18 | 18 | 158 | 0 |  |
| quint | 18 | 18 | 277 | 0 |  |
| ocaml | 18 | 18 | 373 | 50 |  |
| isabelle | 18 | 18 | 331 | 0 |  |
| idris | 18 | 18 | 730 | 47 |  |
| pkl | 18 | 18 | 482 | 59 |  |
| veil | 17 | 18 | 244 | 0 | sessions.lockedInactive |

- readers are fresh subagents given no description of the syntax; comments are stripped

## 道具の費用（cost、automatic）

書く量と検査の時間はどれだけか。得点: 候補の中で最小の行数を1とした比（最小/自分）。

| 候補 | modelLines | sharedLines | checkSeconds |
| --- | --- | --- | --- |
| rmodel | 124 | 0 | 1.30 |
| quint | 243 | 0 | 1.20 |
| ocaml | 166 | 170 | 2.90 |
| isabelle | 256 | 38 | 9.90 |
| idris | 394 | 299 | 2 |
| pkl | 166 | 279 | 0.10 |
| veil | 210 | 0 | 40.70 |

- lines exclude blank and comment lines; shared DSL or schema lines are reported, not scored
