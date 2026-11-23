# 0075: 線形の連鎖を checker の検証と評価で 1 段の入れ子として数える

[#82](https://github.com/mizchi/idealis/issues/82)の 2 回目の iteration で、[#83](https://github.com/mizchi/idealis/issues/83)も同じ iteration で扱った。[事前plan](../plans/0075-nesting-chains-ir-eval.md)を 9488851d6 を基準に記録し、実装の段で変えた条件を評価の前に plan に追記した。push と remote merge は含めない。

rtlc の v2 で、0074 で両 frontend が 1 段と数えるようにした連鎖を、checker の検証と評価でも 1 段と数える。IR・wire・両 frontend・v1 は変えていない。今受理される入力の IR は byte 単位で同じで、結果と work も同じだった。#83 は、評価の再帰で積まれる frame を debug で小さくし、9 種類の入れ子が debug の既定 2 MiB の thread で動的評価 128 に届くようにした。#82 と #83 の両方を採用する。

## 採用した変更

- 連鎖は、`and` の `left` が直接 `and` であるもの（`or` も同じ）と、`if` の `alternative` が直接 `if` であるもの。`right` 側と `consequent` 側の入れ子、`implies`、時間論理の連言・選言、action の branch は段ごとに数えたままにした。
- 連鎖は明示的な loop でたどる。連鎖の先頭で 1 段進め、各項・各条件・各枝・最後の `alternative` はその深さで評価・検証する。対象は式の評価、遅延関数、membership の IF、v2 の構造の検証、metadata の解析の 5 か所（`crates/rtlc/src/safety_chains.rs`、`safety_expressions.rs`、`safety_lazy.rs`、`safety_definitions.rs`）。
- 連言は最も左の項から評価して偽で止まり、選言は真で止まる。IF 連鎖は条件を先頭から評価し、最初に真になった枝だけを評価する。左の spine は 32 項までを固定長の buffer で扱い、超えたときだけ確保する。IF 連鎖の loop は確保しない。2 項の `and` / `or` は今の経路のまま。
- v1 の検証（`validate_with` の 48）は変えていない。評価の loop は v1 でも通るが、v1 には動的評価の上限がなく、結果は変わらない。
- v2 の検証は、式の木の根からの IR の深さが 128 を超える入力を `Safety IR expression nesting exceeds 128` で拒否する（評価の前に plan へ追記した条件）。連鎖を 1 段と数えると、Rust の API で直接組んだ IR が深いまま検証を通り、その後の段ごとに再帰する処理（clone、membership の登録など）で stack を使い切りうるため。frontend の出力検査と JSON・TS の decoder も 128 なので、frontend と wire を通る入力の結果は変わらない。
- #83: `eval_operator` の 26 の腕と `eval_scalar_operator` の 12 の腕を、それぞれ別の関数に分けた。`eval` の `#[inline(always)]` は変えていない。分けた関数には `#[inline]` を付けた。
- metadata の解析の cache の key には IR の深さを含めない（評価の途中で、小さいモデルの `check` の後退を直した変更。Criterion の節）。

## テストと Red

実装の前に次のテストを置き、before（9488851d6 の source にテストだけを加えたもの）で意図した理由で失敗することを記録した（[audit](../runs/0075-audit/)）。

- `crates/rtlc/tests/nesting_chains.rs`（9 件、IR を直接組む）: before で 7 件が失敗した。理由はどれも `Safety expression nesting exceeds 48` か `Safety evaluation nesting exceeds 128`。連鎖でない入れ子の 48 と v1 の 2 件は before でも通る対照。期待値は、同じ形の短い連鎖（before が受理する長さ）の結果を、長い連鎖の期待値とした。work は call・frame と引数の要求でしか数えないので、連鎖の長さによらない。
- `crates/rtlc/tests/evaluation_stack.rs`（9 件、別の test target）: before の debug は `fatal runtime error: stack overflow` で test binary ごと abort した（SIGABRT）。release の before は 9 件とも通った。after は debug・release とも 9 件通った。種類ごとの大きさと結果は、テストを書く前に release の before で測った値。
- `crates/rtlc/tests/deep_drop.rs` の `deep_drop_expression_if_alternative`: 10 万段の ELSE IF は連鎖の形なので、期待値を IR の深さの拒否に変えた。before では checker 48 で失敗する。
- `crates/rtla-compiler/tests/nesting_chains.rs`（14 件）: before で 3 件が失敗した（checker 48 が 65 か所、動的評価 128 が 3 か所）。0075 の入力も期待値と照合する。期待値は native の経路の出力なので、テストは compile した program を native と同じ形の要求の JSON にして、serde_json の既定の入れ子の上限 128 で読めるかを先に確かめる。
- `tests/nesting-chains/contract-test.ts`（363 件）: before で 71 件が失敗し、どれも失敗の詳細に checker 48 か動的評価 128 の拒否を含んでいた。after では全件通った。

## 入力と期待値

0074 の 265 入力（cases.json の 240 件と group `limit` の 25 件）に、plan の 81 入力（`sources-0075.ts`）を加えた。

- 長さ: 平たい連言・選言の 100・127・128・129 項、CASE の 100・126・127・128 枝。括弧つき・IF の 47〜62 は 0074 の入力にある。
- 区切った連鎖: a 枝の IF 連鎖と a 項の連言を持つ定義を d 個連ねたもの（plan の 9 組 × 2）。
- 意味の対照: 60 項の平たい連言・選言と 54 枝の IF 連鎖の位置 1・27・28・54 に、偽、偽による後の 0 除算の短絡、後の偽より先の 0 除算、非 Boolean の項、選言の真による短絡、選んだ枝と未選択の枝の 0 除算、非 Boolean と 0 除算の条件を置いた。
- 位置の対照: membership の位置の 54 枝の IF 連鎖、遅延関数の本体の 54 枝の IF 連鎖、再帰する定義の本体に 54 枝の IF 連鎖を置いたものと置かないもの（3・30・40・60 から数え下げる）。
- 変えない境界の対照: `=>` の連鎖 47〜49、右結合の選言 47〜49、引数のない定義の 127〜129 段の呼び出し。

期待値は `expectations-0075.ts` が導く。before が checker 48 か動的評価 128 で拒否した v2 の連鎖のセルは、定数を外した before の複製（[oracle](../runs/0075-oracle/)）の出力とした。再帰する定義に連鎖を置いた入力は、連鎖を置かない同じ入力の before の出力とした。それ以外は before の出力。導くのに使った 4 つの出力は `fixtures/nesting-chains/records-0075.json` に置き、contract test が同じ規則で導き直して一致を確かめる。

## 4 経路と before / after

350 入力（Quint の 4 件を含む）を、v1 / v2 と 4 経路（TS → Wasm、TS → native、Rust の Wasm、Rust の native）で before・after・定数を外した複製の 3 つに流した。各 2,800 セル（[paths](../runs/0075-paths/)）。

- before と after で、IR の sha256 が異なるセルは 0。両 frontend の正規化した IR は、両方が compile できる 337 セルで一致した（before と同じ集合）。
- v1 は before と after で差が 0 セル。
- v2 で before と変わったのは 222 セルで、すべて before が checker 48（210 セル）か動的評価 128（12 セル）で拒否していたセルだった。after での内訳は complete 144、違反 34、bounded / evaluation 4、`Expected a boolean` 24、`Integer division by zero` 16。許す差の外の差は 0。
- 変わった 222 セルのうち 214 セルは、定数を外した複製と work とエラーの文言まで一致した。残る 8 セルは再帰する定義に連鎖を置いた入力で、複製は連鎖の各段を数えるため 10 work で `bounded / evaluation` になる。after は、連鎖を置かない同じ入力と同じ結果（3・30 から complete、40・60 から bounded / evaluation、work も一致）になった。plan で許した再帰の経路の差として一覧にした。
- after の結果は、Rust frontend の Wasm 経路の v1 の 5 セルを除いて、記録した期待値と一致した。5 セルは平たい 128・129 項と CASE 128 枝の v1 で、Wasm の compile の応答を TS の decoder が `Invalid compiled expression` で拒否する。native ではその後の要求の decoder で拒否する。この差は before にもあり、contract test は 5 セルを明示して検査する。同じ frontend の native と Wasm で結果が異なるセルは、この 5 セルだけだった（before・after・複製とも）。
- Quint の LongIfChain は、after で safety が complete 56 状態 / 57 生成 / work 226、broken が trace 54 の違反（work 216）になった。

最上位の invariant に置いたときの受理の範囲（Rust の native と Wasm、TS の両経路）:

| 連鎖                          | 受理      | 拒否                                                                   |
| ----------------------------- | --------- | ---------------------------------------------------------------------- |
| 括弧つきの連言・選言、IF 連鎖 | 61 段まで | Rust は 62 段で parser 64、TS は 62 段も受理                           |
| 平たい連言・選言              | 100 項    | 127・128 項は要求の JSON decoder 128、129 項は frontend の出力検査 128 |
| OTHER つきの CASE             | 100 枝    | 126・127 枝は要求の JSON decoder 128、128 枝は frontend の出力検査 128 |

Rust の API（JSON を通さない）では、平たい 128 項と CASE 127 枝も受理した（rtlc の `nesting_chains` と stack の測定）。区切った連鎖は 18 入力すべてが受理された。before は IF 連鎖の 3 × 41、4 × 31、5 × 25 を動的評価 128 で拒否していた。連言の 9 組は before でも受理されていた。

## TLC との比較

固定 TLC 1.7.4（jar の sha256 は plan の値）を `-Xmx256m -Xss8m -workers 1` で、新しい 81 入力に 1 つずつ実行した。ケースごとに scratch と metadir を分けた（[TLC](../runs/0075-tlc/)）。0074 の 265 入力は `source` と `config` が 0074 の記録と同じで、そのうち TLC を流した 240 件と Quint の 4 件は [0074-tlc](../runs/0074-tlc/) の verdict を引用した。

- 最初の実行では、区切った連鎖と呼び出しの対照の 21 入力が SANY に拒否された。定義を使う前に置いていなかったため。idealis は前方参照を受理する。21 入力を、使う定義を先に置く形に直し、両 baseline・期待値・4 経路・TLC を流し直した。期待値は直す前と同じだった。直す前の記録は [audit](../runs/0075-audit/) に残した。
- 新しい 81 入力の TLC の verdict は complete 53、違反 12、0 除算 8、非 Boolean 8。after の v2 の Rust の native は 59 入力で状態数・生成数・違反の trace の長さ・エラーの種類が TLC と一致した（before は 17）。残りは各層の上限による型付きの拒否（要求の JSON decoder 128 が 6、frontend の出力検査 128 が 3、動的評価 128 が 3、frontend 40 が 3、parser 64 が 3）と、再帰の `bounded / evaluation` 4 件（TLC は complete）。
- TS frontend の v2 は 31 入力で一致した。ほかは、TS が `\div` と RECURSIVE を持たないことによる拒否と、上の各層の上限。
- v1 では再帰する定義の 8 入力を両 frontend が compile で拒否する（v1 は再帰を持たない）。before と after で同じ。
- 0074 の入力で before と after が変わった v2 のセル（Rust 23、TS 26）は、すべて引用した TLC の verdict と一致した。
- Quint の LongIfChain は TLC と同じ safety complete 56 / 57、broken trace 54 の違反。LongConjunction は 0074 と同じ。

## stack

plan の harness（`evals/experiments/nesting-chains/stack-harness/`）を before と after の debug と release で build し、`check` を指定した大きさの thread で流して、1 GiB と同じ出力になる最小の stack を 4 KiB の分解能で二分探索した（[stack](../runs/0075-stack/)、rustc 1.100.0、aarch64-apple-darwin）。単位は KiB。

連鎖（最上位の invariant、v2）:

| 連鎖       |   n | debug before | debug after | release before | release after |
| ---------- | --: | -----------: | ----------: | -------------: | ------------: |
| IF         |   3 |          131 |          51 |              7 |             7 |
| IF         |  47 |        1,283 |          51 |             51 |             7 |
| IF         |  61 |   checker 48 |          51 |     checker 48 |             7 |
| 平たい連言 |   3 |          115 |          51 |              7 |             7 |
| 平たい連言 |  48 |        1,283 |          51 |             51 |             7 |
| 平たい連言 | 128 |   checker 48 |          51 |     checker 48 |             7 |
| CASE       |   3 |          131 |          51 |              7 |             7 |
| CASE       |  47 |        1,283 |          51 |             51 |             7 |
| CASE       | 127 |   checker 48 |          51 |     checker 48 |             7 |

after の最小 stack は n によらず debug 51 KiB、release 7 KiB で、n の差あたりの増分は debug・release とも 0 KiB / 段（plan の上限は debug 1 KiB / 段、release 0.1 KiB / 段）。before が受理する長さで、after が before を超えるセルはない。

#83 の 9 種類（受理される最大の大きさと次の大きさ。どちらも release の before と同じ結果）:

| 種類             |    大きさ | 結果                |  debug before |   debug after | release before | release after |
| ---------------- | --------: | ------------------- | ------------: | ------------: | -------------: | ------------: |
| 式の演算子       | 121 / 122 | complete / 拒否 128 | 3,203 / 3,235 |     659 / 659 |      147 / 147 |     147 / 147 |
| 値の演算子       |   60 / 61 | 同上                | 6,403 / 6,483 |     723 / 707 |      179 / 179 |       51 / 51 |
| call             | 127 / 128 | 同上                |     419 / 387 |     419 / 387 |        35 / 35 |       35 / 35 |
| membership       | 120 / 121 | 同上                | 1,171 / 1,043 | 1,187 / 1,011 |      115 / 115 |       99 / 99 |
| 量化の束縛       | 115 / 116 | 同上                |     467 / 451 |     275 / 259 |        51 / 51 |       51 / 51 |
| ENABLED の step  | 119 / 120 | 同上                |     931 / 931 |     931 / 931 |        67 / 67 |       67 / 67 |
| binder の locals | 115 / 116 | 同上                |     339 / 339 |     339 / 339 |        51 / 51 |       51 / 51 |
| 遅延関数         |   60 / 61 | 同上                | 1,011 / 1,043 |     963 / 995 |        51 / 51 |       51 / 51 |
| 時間論理         | 122 / 123 | 同上                |     595 / 595 |     595 / 595 |        35 / 35 |       35 / 35 |

after の debug の最小 stack は最大で membership の 1,187 KiB で、plan の 1,920 KiB 以下を満たした。before で上限を超えていたのは式の演算子（3,203 KiB）と値の演算子（6,403 KiB）で、腕の分割で 659 KiB と 723 KiB になった。ほかの 7 種類は before でも 1,920 KiB 以下で、`eval_operator` と `eval_scalar_operator` を通らない経路では値がほとんど変わらない（membership は検証の変更で 16 KiB 増えた）。

## 既存の入力と完全応答

- 既存の入力の IR: 0074 と同じ手順で集めた 4,676 入力を、before と after の TS・Rust Wasm・Rust native で v1・v2 ともに compile した。28,056 セルで program の sha256 か拒否の文言がすべて一致した（[IR](../runs/0075-ir-corpora/)）。frontend と IR を変えていないことの確認になる。
- 完全応答: `evaluator-corpus`（Quint6・TCommit・共有 chain、8 経路・各 4 iteration）の 264 比較が、program・結果・work を含めて一致した（[corpus](../runs/0075-corpus/)、[summary](../runs/0075-corpus-summary/)）。
- 時間論理: `temporal-parity` の旧版 156 モデル・1,560 要求が、before・after の native・Wasm で一致した（[temporal](../runs/0075-temporal-parity/)）。

## Criterion

0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で、0074 と同じ driver・14 関数の kernel と、rtla-compiler の 6 本の bench（`quint_generated`・`frontend`・`frontend_semantics`・`compatibility`・`recursive_actions`・`nesting_chains`）を流した。`nesting_chains` には括弧つきの連言と IF 連鎖の n = 3, 8, 47 の `check` を足し、before と after に同じ source で置いた。

- ABBA-1（144 関数）では 10% 以上の後退は 0 件だったが、`frontend-check_except-zero-arity` +9.4%、`recursive-check_accumulator` +9.2% など、小さいモデルの `check` が 8〜9% 遅くなっていた。
- 同じ 3 本の bench を ABBA-2 で流し直すと同じ関数が再び 7〜10% 遅く、`frontend-check_except-zero-arity` は +10.3% だった。再現する後退として扱った。
- 原因を切り分けた。v2 の検証だけを before に戻した複製では、同じ関数の後退は +3.3% 以下になった。後退は、metadata の解析の cache の key に IR の深さを含めたことによる。IR の深さは同じ式の木では経路ごとに 1 つに決まり、解析の cache は呼び出しごとに作り直すので、key から外しても判定は変わらない。key から外し、分けた腕に `#[inline]` を付けた（`#[inline]` だけでは後退は残った）。
- この修正の後、after の binary を作り直し、after を使う評価（4 経路、TLC との比較の集計、既存の入力の IR、完全応答、時間論理、kernel、stack）をすべて流し直した。結果は修正の前と同じで、stack は membership の 120 段が debug で 1,171 KiB から 1,187 KiB になっただけだった。
- 修正後の ABBA-3（144 関数）: 10% 以上の後退は 0 件。最大は `frontend-check_factorial` の +6.2% で、変化の中央値は +0.3%。`nesting_chains` の `check` は if-chain-47 が -43%、paren-and-47 が -27%、if-chain-8 が -10%。
- kernel（14 関数）: -9.2% から +0.0%。10% 以上の後退はない。

[ABBA-1](../runs/0075-abba-1/)、[ABBA-2](../runs/0075-abba-2/)、[ABBA-3](../runs/0075-abba-3/)、[kernel](../runs/0075-kernel-summary/)を参照。ABBA-1 と ABBA-2 は修正の前の after で、判断には ABBA-3 を使う。

## 検査

- `just check`（書式、型、Vitest 79 件、node:test 79 件）。最初の実行は新しい 6 ファイルの書式で失敗し、`vp check --fix` の後に通った。
- `just wasm-test`（23 件）、`just safety-test`（1,687 件。新しい contract を含む）、`just quint-test`（38 件）。
- `just portable-build` と `just portable-test`（`rtlc-test`・`rtla-test`・`compiler-test` の fmt・clippy `-D warnings`・全テストを含み、Rust 610 件と node:test 33 件）。
- #83 のテストは debug（`cargo test`）と release（`cargo test --release`）の両方で通った。
- `just browser`（Chromium 7 件）、`just evaluate` と `just replay-evaluation`。
- `just support-check`。
- TS のソース（`packages/idealis/src`）は変えていないので、`just mutation` は対象外とした。

記録した run では、実行したホストの絶対パスとユーザー名を、記録する時点で placeholder に置き換えた。Criterion の sample、ケースごとの出力、log は消し、消したファイルの数・バイト数・一覧の digest を各 run の `reduced.json` に残した（[audit](../runs/0075-audit/)）。

## 残る制約

- IR の深さは今も連鎖の長さに比例する。要求の JSON decoder 128 のため、平たい連言・選言は 127 項、CASE は 126 枝から、JSON を通る経路で型付きに拒否される。129 項以上は frontend の出力検査 128 で拒否される。これを超える連鎖が実際の入力で必要になったら、plan の候補 3（n 項の `And` / `Or` と cond の連鎖を IR v2 に加える）が次の候補になる。
- Rust の parser が最も外側まで括弧で囲んだ 62 段以上の連鎖を拒否し、TS が parse する差は、0074 から変えていない。
- 変えない境界の対照のうち、TLA+ で書いた `=>` の連鎖と右結合の選言は frontend 40 と parser 64 で、引数のない定義の 127〜129 段の呼び出しは動的評価 128 で、どれも checker の 48 の手前か後で止まった。before と after で同じ結果だったが、checker の 48 と動的評価の 127 / 128 / 129 の境界は、rtlc の IR を直接組んだテストと既存の unit test が検査している。
