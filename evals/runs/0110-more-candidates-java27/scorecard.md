# 成績表（0110-more-candidates）

各軸は0〜1。—は未測定（合格に数えない）。1つの点にまとめない（重みなし）。

| 候補 | DDD記述 | 形式化との相性 | 機械での理解容易性 | 人間への理解容易性（代理） | 道具の費用 | 実装への接続 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| rmodel | 0.64 | 0.88 | 1 | — | 0.76 | 0.83 |
| quint | 0.79 | 0.25 | 1 | — | 0.39 | 0.17 |
| ocaml | 0.93 | 0.13 | 0.50 | — | 0.57 | 0.33 |
| isabelle | 0.93 | 0.13 | 0.75 | — | 0.37 | 0.33 |
| idris | 0.93 | 0.13 | 0.50 | — | 0.24 | 0.33 |
| pkl | 0.86 | 0.53 | 1 | — | 0.57 | 0.17 |
| veil | 0.79 | 0.38 | 0.75 | — | 0.45 | 0 |
| souther | 0.86 | 0 | 0.75 | — | 0.38 | 0.83 |
| koine | 0.50 | 0 | 1 | — | 0.66 | 0.67 |
| contextmapper | 0.64 | 0 | 1 | — | 1 | 0.33 |
| fizzbee | 0.57 | 0.13 | 1 | — | 0.72 | 0.33 |
| p | 0.57 | 0.13 | 0.75 | — | 0.26 | 0.50 |

## DDD記述（ddd、rubric）

DDDの概念をそのまま書けるか。得点: DDDの概念7つ（直接1・回避0.5・書けない0）の平均。

| 候補 | identity | config | ints | adt | typeExclusion | step | lone |
| --- | --- | --- | --- | --- | --- | --- | --- |
| rmodel | 1 | 1 | 1 | 0.50 | 0 | 0 | 1 |
| quint | 0.50 | 0.50 | 1 | 1 | 1 | 0.50 | 1 |
| ocaml | 1 | 1 | 0.50 | 1 | 1 | 1 | 1 |
| isabelle | 1 | 1 | 1 | 1 | 0.50 | 1 | 1 |
| idris | 1 | 1 | 0.50 | 1 | 1 | 1 | 1 |
| pkl | 1 | 1 | 1 | 1 | 0 | 1 | 1 |
| veil | 1 | 1 | 0.50 | 0.50 | 1 | 1 | 0.50 |
| souther | 0.50 | 1 | 0.50 | 1 | 1 | 1 | 1 |
| koine | 0.50 | 0.50 | 0.50 | 0.50 | 0 | 0.50 | 1 |
| contextmapper | 0.50 | 0.50 | 1 | 0.50 | 0.50 | 0.50 | 1 |
| fizzbee | 0.50 | 0.50 | 1 | 0.50 | 0 | 1 | 0.50 |
| p | 0.50 | 0.50 | 0.50 | 0.50 | 0.50 | 1 | 0.50 |

- rubric: evidence from each candidate's model and writer report
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
| souther | 0 | — | — |  | 0 |
| koine | 0 | — | — |  | 0 |
| contextmapper | 0 | — | — |  | 0 |
| fizzbee | 0 | — | — | complete-search | 1 |
| p | 0 | — | — | bounded | 0.50 |

- verification kinds: rubric from the candidate's checks (complete-search, bounded, inductive, proof)

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
| souther | 0.50 | passed |
| koine | 1 | passed |
| contextmapper | 1 | passed |
| fizzbee | 1 | passed |
| p | 0.50 | passed |

- tree: rubric (1 an analysable tree, 0.5 analysable only inside the host, 0 host functions)
- toolchain: the candidate's check commands, run in this run

## 人間への理解容易性（代理）（human、agent）

構文の説明なしで、読み手が振る舞いを読み取れるか。得点: 新しい読み手の、振る舞いの問いの正答率（本物の人間ではない）。

| 候補 | correct | questions | readers |
| --- | --- | --- | --- |
| rmodel | — | 15 | 0 |
| quint | — | 15 | 0 |
| ocaml | — | 15 | 0 |
| isabelle | — | 15 | 0 |
| idris | — | 15 | 0 |
| pkl | — | 15 | 0 |
| veil | — | 15 | 0 |
| souther | — | 15 | 0 |
| koine | — | 15 | 0 |
| contextmapper | — | 15 | 0 |
| fizzbee | — | 15 | 0 |
| p | — | 15 | 0 |

- readers are fresh subagents given no description of the syntax; comments are stripped
- rmodel: 0 of 2 readers collected
- quint: 0 of 2 readers collected
- ocaml: 0 of 2 readers collected
- isabelle: 0 of 2 readers collected
- idris: 0 of 2 readers collected
- pkl: 0 of 2 readers collected
- veil: 0 of 2 readers collected
- souther: 0 of 2 readers collected
- koine: 0 of 2 readers collected
- contextmapper: 0 of 2 readers collected
- fizzbee: 0 of 2 readers collected
- p: 0 of 2 readers collected

## 道具の費用（cost、automatic）

書く量と検査の時間はどれだけか。得点: 候補の中で最小の行数を1とした比（最小/自分）。

| 候補 | modelLines | sharedLines | checkSeconds |
| --- | --- | --- | --- |
| rmodel | 124 | 0 | 1.10 |
| quint | 243 | 0 | 1.20 |
| ocaml | 166 | 170 | 1.80 |
| isabelle | 256 | 38 | 10 |
| idris | 394 | 299 | 1.30 |
| pkl | 166 | 279 | 0.10 |
| veil | 210 | 0 | 40.10 |
| souther | 245 | 0 | 1.30 |
| koine | 143 | 0 | 3.80 |
| contextmapper | 94 | 0 | 3.20 |
| fizzbee | 131 | 0 | 6.80 |
| p | 359 | 0 | 3.50 |

- lines exclude blank and comment lines; shared DSL or schema lines are reported, not scored

## 実装への接続（impl、rubric）

モデルから実装の言語へ繋がるか。得点: 生成・照合・不変条件の強制の3つ（rubric）の平均。

| 候補 | codegen | conformance | enforcement |
| --- | --- | --- | --- |
| rmodel | 1 | 1 | 0.50 |
| quint | 0 | 0.50 | 0 |
| ocaml | 0.50 | 0 | 0.50 |
| isabelle | 0.50 | 0 | 0.50 |
| idris | 0.50 | 0 | 0.50 |
| pkl | 0.50 | 0 | 0 |
| veil | 0 | 0 | 0 |
| souther | 0.50 | 1 | 1 |
| koine | 1 | 0 | 1 |
| contextmapper | 0.50 | 0.50 | 0 |
| fizzbee | 0 | 1 | 0 |
| p | 0 | 1 | 0.50 |

- codegen: generates code for implementation languages (2+ 1, one 0.5, none 0)
- conformance: checks an implementation against the model (examples or traces 1, partly 0.5, none 0)
- enforcement: enforces invariants on the implementation side (generated code or types 1, run-time checks only 0.5, none 0)
