# 0075: 線形の連鎖を checker の検証と評価で 1 段の入れ子として数える

[#82](https://github.com/mizchi/idealis/issues/82)の 2 回目の iteration で、[#83](https://github.com/mizchi/idealis/issues/83)も同じ iteration で扱う。基準は 9488851d6（origin/main）。[0074](../iterations/0074-nesting-chains.md)は両 frontend で連鎖を 1 段と数え、frontend の 40 を外した。その後も式の位置の連鎖は IR の深さが長さに比例し、rtlc の v2 の静的検証 48 と動的評価 128 で止まる。この iteration では rtlc の v2 だけを変え、同じ連鎖を検証と評価でも 1 段と数える。IR・wire・両 frontend と、v1 の数え方と結果、上限の値（frontend 40、checker の式 48、動的評価 128、frontend の出力検査 128、JSON decoder と TS decoder の 128、parser 64）は変えない。#83 では、評価の再帰の 1 段あたりの stack を debug で減らし、上限 128 の内側で型付きの拒否を返すようにする。push と remote merge は含めない。

## 観測

9488851d6 で測った。入力は 0074 の公開用の合成モデルと、同じ形の TLA+（`EXTENDS Integers`、`Inv == <連鎖>`、Init / Next は 0 から 5 を回る 1 変数）だけを使う。0074 の記録のとおり、手元のモデルにも 54 枝の IF 連鎖と同じ形がある。

既存の `crates/rtla-compiler/tests/nesting_chains.rs` の 12 件は通る。固定版 Quint 0.33.1 と Apalache 0.63.0 の生成物（`fixtures/nesting-chains/quint/`、LongConjunction.tla の sha256 は da139c2f1461064a3197165c315c7aa136221184e67c6473e33ed2d4fa9b3954、LongIfChain.tla は 335653adbc2afd1efacd0628465f31a054b68c66f771e3dea0b2dacfcb2dcc98）では、44 項の連言が v2 で complete 6 状態 / 7 生成 / work 20 になる。54 枝の IF 連鎖は compile を通り、safety・broken とも検査時に `Safety expression nesting exceeds 48` で拒否される。固定 TLC 1.7.4 は IF 連鎖の safety を complete 56 状態 / 57 生成、broken を trace 54 の違反とする（[0074-tlc](../runs/0074-tlc/)）。

Rust の API（parse → compile → `SafetyProgram::check`、1 GiB の thread）で、最上位の invariant に置いた長さの境界を測った。

| 連鎖（v2）           | 受理する最大 | 次の長さでの拒否                                             |
| -------------------- | -----------: | ------------------------------------------------------------ |
| 括弧つき左結合の連言 |           48 | 49 で checker 48                                             |
| IF / ELSE IF         |           47 | 48 で checker 48（54 も同じ）                                |
| 平たい連言           |           48 | 49〜128 で checker 48、129 で frontend の出力検査 128        |
| OTHER つきの CASE    |           47 | 48〜127 で checker 48、128 で frontend の出力検査 128        |
| 括弧つき・IF の共通  |           61 | 62 で parser（`TLA parsing: Expression nesting exceeds 64`） |

v1 は 0074 の記録と同じだった（括弧つき連言 24 項まで、CASE 47 枝まで、平たい連言は 48 項で checker 48）。

連鎖を定義の呼び出しで区切ると、48 は定義ごとなので通り、動的評価の 128 で止まる。`D0(k) == IF x = 20 THEN FALSE ELSE ... ELSE D1(k)` のように、a 枝の IF 連鎖を持つ定義を d 個連ねた入力で測った。

| d   | 受理する最大の a | 次の a での拒否                               |
| --- | ---------------: | --------------------------------------------- |
| 2   |               47 | 48 で checker 48                              |
| 3   |               40 | 41 で `Safety evaluation nesting exceeds 128` |
| 4   |               30 | 31 で同上                                     |
| 5   |               24 | 25 で同上                                     |

評価の深さは、おおよそ d ×（a + 1）+ 3 になる。3 × 40 の入力は、release で complete 6 状態 / 7 生成 / work 38 になる。

上限の場所と、守っているもの。

- 動的評価 128: `crates/rtlc/src/safety_expressions.rs:113-127`（`EvalContext::descend`）。`eval`（同 808-830）は葉を除く全ての node で `eval_nonleaf`（同 832）から `descend` を通る。ほかに membership の domain（同 1663-1695）、量化の束縛（`visit_bindings`、同 1890-1898）、branch の step（同 1968-1975）、binder の locals（同 2268-2277）、遅延関数（`crates/rtlc/src/safety_lazy.rs:59-`）、時間論理の lowering（`crates/rtlc/src/temporal.rs:128-137`）が通る。metadata・定義・frame のどれもない program（v1）では、`descend` は何もしない（同 114-116）。守るのは、定義の呼び出しと再帰を重ねたときの評価の stack。再帰の経路の超過は `bounded / evaluation`、それ以外は protocol エラー。
- checker 48（v2）: `crates/rtlc/src/safety_definitions.rs` の、明示的な stack で IR をたどる構造の検証（223-225 の式、259-261 の branch）、action frame の展開数（79-81）、再帰で走る metadata の解析 `expression_uncached`（642-648）。式の位置の連鎖は 1 段ごとに深さが 1 増える。守るのは、定義の中の式をたどる再帰の stack。
- checker 48（v1）: `safety_expressions.rs:643-646` の `validate_with`。v1 には動的評価の上限がないので、v1 の評価の stack はこの 48 だけで守られている。
- 境界: frontend の出力検査 128（`crates/rtla-compiler/src/typed.rs:282-290`）、TS decoder の 128（`packages/idealis/src/safety-program.ts:166-168`）、要求 JSON の decoder の再帰 128（`crates/idealis-core/src/lib.rs:587`、serde_json の既定）。
- 評価の work: `And`・`Or`・`If` の評価（`safety_expressions.rs:1020-1037`）と `descend` は `Arena::consume` も `tick` も呼ばない。work は call・frame の入口と引数・capture の要求で数える。

### stack（#83）

評価の段の stack を、上限を外さずに測った。harness は rtla・rtla-compiler・rtlc を path で参照する外部の crate で、release は opt-level 3・LTO・codegen-units 1・panic=abort、debug は既定に panic=abort を加えた。program を 1 GiB の thread で parse・compile し、`check` だけを指定した大きさの thread で動かす。出力が complete になる最小の stack を 4 KiB から 1 GiB の間で二分探索した（分解能 4 KiB）。rustc 1.100.0、aarch64-apple-darwin。

| 入力                   | 評価の深さ（概算） | release | debug |
| ---------------------- | -----------------: | ------: | ----: |
| 1 枝の定義 1 個        |               約 5 |       7 |    83 |
| 最上位の IF 連鎖 3 枝  |               約 5 |       7 |   131 |
| 最上位の IF 連鎖 47 枝 |              約 49 |      51 | 1,283 |
| 38 枝の定義 1 個       |              約 42 |      35 | 1,043 |
| 38 枝の定義 2 個       |              約 81 |      83 | 2,035 |
| 38 枝の定義 3 個       |             約 120 |     131 | 3,027 |
| 30 枝の定義 4 個       |             約 127 |     147 | 3,187 |

単位は KiB。IF の 1 段あたり、release は約 1.0〜1.2 KiB、debug は約 25〜26 KiB になる。#83 の記録（上限だけを外した複製で、release 約 0.63 KB、debug 約 26.5 KB）と、debug ではほぼ同じ値になった。#83 は prepare を済ませた program の評価を測り、ここでは prepare と検証を含む `check` を測ったので、release の値は直接比べない。上限 128 の直前の約 127 段で、debug は 3,187 KiB を使う。Rust のテストが既定で使う 2 MiB の thread では次のようになる。

- 38 枝の定義 2 個（約 81 段）は debug で通った。必要な 2,035 KiB と 2 MiB の差は 13 KiB しかない。#83 ではこの入力が abort したと記録しており、呼び出し元の frame の違いで結果が変わる境目にある。
- 38 枝の定義 3 個と 30 枝の定義 4 個は、debug で `fatal runtime error: stack overflow` により abort する。release ではどちらも complete になる。
- 31 枝の定義 4 個は、release では `Safety evaluation nesting exceeds 128` の型付きの拒否になり、debug では拒否の前に abort する。

1 段あたりの量は node の種類で変わる。1 枝の定義（call・IF・等号）を 1、2、4 個連ねると、debug で 83、115、179 KiB だった。既存の rtlc の unit test は、127 段の call の連鎖を debug の既定の thread で通している。

## 候補

簡単な順に並べる。どれも、TLC に揃えた評価順・短絡・最初に報告するエラー・work を変えないことを前提にする。

1. **上限の値を実測から決め直す。** 定数だけを変えるので最も簡単。ただし、上の測定では両立する値がない。v2 の 48 を上げると release では 54 枝が通るが、受理できる長さは今も連鎖の長さに比例し、次に長い連鎖で同じ拒否に戻る。debug では 47 枝の IF 連鎖がすでに 1,283 KiB を使う。動的評価 128 を debug の 2 MiB に収まる値（約 75 段）まで下げると、release で complete になる 38 枝 × 3 定義（約 120 段）を新たに拒否する。build の profile ごとに上限を変えると、同じ入力の結果が debug と release で変わり、native・Wasm・debug のテストの照合が成り立たない。採らない。1 段あたりの実測は、残す上限の根拠として記録する。
2. **線形の連鎖を、rtlc の v2 の検証と評価で反復してたどる（採る）。** IR は二項のまま変えず、checker が連鎖を 1 段と数える。変更は rtlc の評価と v2 の検証に閉じ、両 frontend・wire・contracts は変えない。今受理される入力の IR は、作り方からして byte 単位で同じになる。今受理される入力では評価の深さが減るだけなので、受理から拒否へ変わる入力はない。残る制約は、IR の深さが長さに比例するため、IR を再帰でたどる境界（frontend の出力検査 128、JSON と TS の decoder 128）で、128 段前後の連鎖が型付きで拒否されること。手元のモデルの長さ（44 項、54 枝）はその内側にある。
3. **n 項の `And` / `Or` と cond の連鎖を IR v2 に加える。** IR の深さも長さに比例しなくなり、decoder と frontend の出力検査の 128 も連鎖の長さで尽きない。代わりに wire・`contracts.ts`・TS decoder・両 frontend の生成・rtlc の全 visitor（検証、metadata、partial order、drop、coverage）を変える。今受理される v2 の入力の IR も変わるので、0074 で使った IR の byte 単位の一致を対照に使えなくなる。0074 の記録はこの案を 0075 の候補としていた。2 で手元のモデルの長さを通せるので、ここでは採らない。128 段を超える連鎖が実際の入力で必要になったときの次の候補として残す。

#83 の候補も簡単な順に並べる。

- a. build の profile ごとに上限を変える。上の 1 と同じ理由で採らない。
- b. 動的評価の上限を debug の 2 MiB に収まる値へ下げる。今受理される入力を拒否するので採らない。
- c. native の `check` が、指定した大きさの stack を持つ thread で評価する。Wasm では thread を使えず、native と Wasm で経路が分かれる。prepared な小さい問い合わせ（[0064](../iterations/0064-prepared-safety.md)）では、呼び出しごとの thread の費用も足される。採らない。
- d. 評価の再帰の経路で、1 段ごとに積まれる関数の frame を debug で小さくする（採る）。`eval_nonleaf`・`eval_scalar_operator`・`eval_operator` の大きな match の、再帰しない腕を別の関数に分け、再帰で積まれる frame に一時値の領域を残さない。意味論は変えない。frame の大きさは compiler の版で変わりうるので、debug と release の両方で上限の直前と直後を検査するテストを置いて守る。
- e. 明示的な stack で評価する、または `stacker` のような依存で stack を伸ばす。評価器の書き直しか、Wasm で動かない依存が要る。採らない。

## 仮説

rtlc の v2 で、連鎖を明示的な loop でたどり、連鎖全体を 1 段の入れ子として数えれば、54 枝の LongIfChain は固定 TLC と同じ 56 状態 / 57 生成で complete になり、broken は trace 54 の違反になる。評価順・短絡・最初のエラー・work は変わらない。さらに、評価の再帰の frame を debug で小さくすれば、どの種類の入れ子でも深さ 128 に達する前に debug の 2 MiB を超えなくなる。

- 連鎖は、`and` の `left` が直接 `and` であるもの（`or` も同じ）、または `if` の `alternative` が直接 `if` であるものとする。0074 で両 frontend が式の位置に出す形と同じ。`right` 側の入れ子と `consequent` 側の入れ子は、今と同じく段ごとに数える。`implies`・時間論理の連言と選言・action の branch は対象外。
- 連鎖の先頭で `descend` を 1 回通し、各項・各条件・各枝・最後の `alternative` は、先頭から 1 段進めた context で評価する。v2 の構造の検証と `expression_uncached` でも、各項に先頭の深さ + 1 を与える。定義の中の式の深さ 48 は、連鎖を 1 段として数える。
- 評価の順は今と同じにする。連言は最も左の項から順に評価し、偽になった時点で残りを評価しない。選言は真で止まる。IF 連鎖は条件を先頭から評価し、最初に真になった枝だけを評価する。非 Boolean の項や条件のエラーは、今と同じ位置で同じ文言になる。
- 連鎖をたどる場所は、式の評価（`eval_scalar_operator`）、遅延関数（`safety_lazy.rs`）、membership の IF（`safety_expressions.rs:1680-1695`）、v2 の構造の検証、metadata の解析の 5 か所。
- 2 項の `and` / `or` と、`alternative` が `if` でない `if` は今の経路のままにし、連鎖の判定以外の費用を足さない。IF 連鎖の loop は確保をしない。左の spine は、確保をしない範囲を固定した長さの buffer で扱い、超えたときだけ確保する。
- v1 は変えない。v1 には動的評価の上限がなく、48 が評価の stack を守る唯一の上限なので、数え方も結果も今のまま。
- #83 の frame の分割は、v1・v2 の両方の評価の経路に当てる。結果・work・エラーは変えない。

予測（結果を見る前に固定する）。v2 で最上位の invariant に置いたとき、括弧つきの連言・選言と IF 連鎖は parser の上限の手前の 61 まで、平たい連言・選言は 128 項まで、CASE は 127 枝まで、Rust の API で受理される。それより長いものは今と同じ層（parser 64、frontend の出力検査 128）で拒否される。JSON の要求と TS の decoder を通る経路では、wire の入れ子が IR の深さより数段深いので、128 の decoder の上限でより短い長さで拒否されうる。その境界は経路ごとに記録し、型付きの拒否であること（abort しないこと）と、同じ frontend の native と Wasm で一致することを要求する。区切った連鎖は、上の表で拒否される 3 × 41、4 × 31、5 × 25 がすべて受理される見込み。

## 固定条件と対照

- before は 9488851d6 の source / binary。after は before にこの変更だけを加えたもの。それぞれの sha256 を記録する。`crates/rtla/src`、`crates/rtla-compiler/src`、`crates/rtlc/src/ir.rs`、`packages/idealis/src`、`crates/idealis-core/src` が before と after と評価の終了時で変わらないことを sha256 で確かめる。変わるのは `crates/rtlc/src` の評価と v2 の検証だけとする。
- テストを先に置き、before で意図した理由で失敗すること（Red）を記録する。連鎖は `Safety expression nesting exceeds 48` と `Safety evaluation nesting exceeds 128`、#83 は debug の 2 MiB の thread での stack overflow。Rust は `crates/rtlc/tests/` に置く。stack overflow は test binary 全体を abort させるので、#83 のテストは別の test target にし、abort をそのテストの失敗として特定できるようにする。TS の 4 経路の contract は `tests/nesting-chains/` に足し、固定入力と期待値は `fixtures/nesting-chains/`、生成・計測・stack の harness は `evals/experiments/nesting-chains/` に置く。
- 期待値が変わる既存のテストを、実装の前にここで宣言する。`crates/rtla-compiler/tests/nesting_chains.rs` の checker 48 の 5 か所（連鎖の長さの閾値と、54 枝の Quint の IF 連鎖）と、`fixtures/nesting-chains/` の期待値のうち v2 の連鎖の綴りが checker 48 か動的評価 128 で拒否されるものは、この変更の目的どおりに変わる。`fixtures/nesting-chains/baseline.json` は before の出力の記録なので変えない。ほかの既存のテスト（`tests/tlc-differential/analysis-ir-corpus.ts` と `tests/workspace-oracle/contract-test.ts` の 48 の期待値、rtlc の unit test の 48 / 128 の境界を含む）は、連鎖の形かどうかを実装の前に分類して記録する。連鎖でない入れ子の期待値は変えない。分類と違うテストが失敗したら、実装を直すか、評価の前に条件の変更として plan に追記する。
- 合成の入力（公開用に作ったもので、特定の対象の名前を含まない。乱択は使わない）:
  - Quint の LongConjunction と LongIfChain（上の sha256）。生成し直さず、固定した生成物を使う。
  - TS と共通の TLA+: 括弧つきの連言・選言・IF 連鎖の n ∈ {47, 48, 49, 54, 61, 62}、平たい連言・選言の n ∈ {48, 49, 100, 127, 128, 129}、CASE の n ∈ {47, 48, 100, 126, 127, 128}。
  - 区切った連鎖: a 枝の IF 連鎖の定義を d 個連ねたもの。(d, a) ∈ {(2, 38), (2, 47), (3, 38), (3, 40), (3, 41), (4, 30), (4, 31), (5, 24), (5, 25)}。定義の中で連言を連ねた同じ形も置く。
  - 意味の対照（n = 54 の IF 連鎖と n = 60 の平たい連言、位置 p ∈ {1, 27, 28, 54}）: 0074 と同じ組（p の偽による違反、偽が後の 0 除算を短絡する違反、p の 0 除算が後の偽より先に起きるエラー、非 Boolean の項と条件、選言の真による短絡、第 p 枝の選択と未選択の枝の 0 除算）。
  - 位置の対照: membership の位置の 54 枝の IF 連鎖、遅延関数の本体に置いた 54 枝の IF 連鎖、再帰する定義の本体に置いた IF 連鎖（`bounded / evaluation` に達する深さを含む）。
  - 変えない境界の対照: `~` と `Id(...)` の入れ子、`/\` と `\/` が交互に入れ子になる式、`right` 側に入れ子になる手書きの IR、`consequent` 側の IF の入れ子、`=>` の連鎖、時間論理の連言、既存の `deep_drop` と手書き IR の 48 / 49、連鎖を含まない動的評価の 127 / 128 / 129、v1 の全入力。before と after で同じ結果を要求する。
- 結果の期待値: status・状態数・生成数・反例は固定 TLC から取る。TLC は 0074 と同じ v1.7.4（revision 5a47802b、jar の sha256 936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88）を `-Xmx256m -Xss8m -workers 1` で 1 つずつ実行し、scratch と metadir をケースごとに分ける。0074 の 244 入力は、入力の sha256 が一致するものだけ [0074-tlc](../runs/0074-tlc/) の結果を引用し、新しい入力だけを実行する。work と評価エラーの文言は、before から 48 と 128 の定数だけを外した複製（観測専用、1 GiB の thread、時間とメモリの上限つき、状態数が有界な入力に限る）で同じ IR を検査した値と一致することを要求する。今受理される入力では、after の結果（work を含む）が before と完全に一致することを要求する。
- 許す差: before で checker 48 か動的評価 128（再帰の経路では `bounded / evaluation`）になっていた入力が、after で受理されるか、後の層で型付きに拒否される場合だけ。再帰の経路が連鎖を通る入力は、`bounded / evaluation` のまま work が増えることも許し、その入力を一覧にして保存する。それ以外の差は 0 件を要求する。
- 経路: Rust frontend の native / Wasm、TS frontend から native / Wasm の 4 経路と v1 / v2 で、before と after を流す。0074 の 244 入力に上の入力を加える。同じ frontend の native と Wasm の結果の一致、両 frontend の IR の正規化した一致を要求する。v1 は差 0 を要求する。frontend を変えないので、両 frontend の IR は before と after で byte 単位で同じになることも要求する。Chromium は `just browser` を 1 回流して記録する。
- 既存の入力の結果: 完全応答の `evaluator-corpus`（Quint6・TCommit・共有 chain、8 経路・各 4 iteration）と、旧版 156 モデル・1,560 要求の `just temporal-parity` を before / after で流し、program・結果・work の完全一致を要求する。
- stack: 上の harness を `evals/experiments/nesting-chains/` に置き、before と after、debug と release で同じ二分探索を流す。段は decode・検証・評価・drop を分ける。
  - 連鎖: IF 連鎖の n ∈ {3, 47, 61}、平たい連言の n ∈ {3, 48, 128}、CASE の n ∈ {3, 47, 127}。after の最小 stack は、before が受理する長さで before を超えないこと。after で n の最小と最大の差を n の差で割った値が、debug で 1 KiB / 段、release で 0.1 KiB / 段 を下回ること（before は debug 約 26 KiB / 段）。
  - #83: `descend` を通る種類ごとに入れ子を作る（式の演算子、集合などの値の演算子、call、membership、量化の束縛、ENABLED の branch、binder の locals、遅延関数、時間論理）。release で受理される最大の大きさと、次の大きさで拒否される入力の組を種類ごとに決め、debug の 2 MiB の thread で両方が release と同じ結果（complete、`bounded / evaluation`、または型付きの拒否）になることを要求する。深さ 127 の評価の debug の最小 stack は 1,920 KiB 以下（2 MiB から 128 KiB の余裕）とする。上の 38 枝の定義 2 個のように 13 KiB の差で通る状態を、合格に数えない。
- Criterion: 0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）。rtlc を変えるので、[0074-kernel](../runs/0074-kernel-summary/)と同じ driver・14 関数の kernel と、rtla-compiler の `quint_generated`・`frontend`・`frontend_semantics`・`compatibility`・`recursive_actions`・`nesting_chains` を流す。`nesting_chains` に、括弧つき連言と IF 連鎖の n = 3, 8, 47 の `check` を足し、before と after の両方に同じ source で置く（before が受理する長さに限る）。10% 以上の後退は同条件の ABBA で再確認し、再現する後退を残したまま採用しない。
- build・test・benchmark は同時に流さない。

## 記録と完了条件

run は追記のみ。`evals/runs/0075-*` に、source / binary / 入力・driver・実行順・予算・host・終了コードを保存し、失敗も残す。README の規則に従い、commit するのは iteration が引用する summary（表、集計、manifest、終了コード、sha256）だけにし、Criterion の sample、ケースごとの出力、log、大きな dump は消した数・バイト数・digest を `reduced.json` に残す。記録する時点で、実行したホストの絶対パスとユーザー名を placeholder に置き換える。

予定する run は `0075-tlc`、`0075-paths`、`0075-oracle`（定数を外した before の複製）、`0075-corpus` / `0075-corpus-summary`、`0075-temporal-parity`、`0075-stack`、`0075-kernel` / `0075-kernel-summary`、`0075-abba-N`、`0075-browser`、`0075-evaluate`、`0075-audit`。

終了前に `just check`、`just wasm-test`、`just safety-test`、`just rtlc-test`、`just compiler-test`、`just quint-test`、`just portable-build` / `just portable-test`、`just evaluate` / `just replay-evaluation` を実行する。#83 のテストは debug（`cargo test`）と release（`cargo test --release`）の両方で流す。`docs/safety-ir-v2.md` の「評価と検証」と連鎖の段落、`crates/rtlc/SUPPORT.md` の動的評価 128 の項目、`crates/rtla-compiler/SUPPORT.md` の未チェックの項目（IR の深さが長さに比例する件）を更新し、`just support-check` を通す。TS のソースは変えないので `just mutation` は対象外とし、その理由を記録する。

次がそろったときに採用する。

- Red から Green。#83 のテストは before で stack overflow、after で debug・release とも上限の直前で受理、直後で型付きの拒否。
- LongIfChain の safety が complete 56 / 57、broken が trace 54 の違反で、固定 TLC と一致する。新しい入力の status・状態数・生成数・反例が TLC と一致し、work とエラーが定数を外した before の複製と一致する。
- 許す差の外で、before と after の結果・work・IR に差がない。v1 は差 0。
- 上の stack の条件を満たす。
- 再現する 10% 以上の後退がない。

#83 の d が、すべての種類で深さ 127 の debug の最小 stack を 1,920 KiB 以下にできない場合は、#82 の部分だけを採否の対象にし、#83 は測った値と残る種類を iteration に記録して開いたままにする。採否と残る制約を iteration と README に戻す。

この変更の後も IR の深さは連鎖の長さに比例するので、frontend の出力検査 128 と JSON・TS の decoder 128 を超える連鎖は型付きで拒否される。Rust の parser が最も外側まで括弧で囲んだ 62 段以上の連鎖を拒否し、TS が parse する差も、0074 から変えない。

## 実装の段で変えた条件（評価の前に追記）

評価を始める前に、実装とテストを置く段で次の条件を変えた。

- **IR の深さの検査を v2 の検証に足した。** 連鎖を 1 段と数えると、Rust の API で直接組んだ IR では、連鎖の長さだけ IR が深くなっても検証を通る。検証の後には、IR を段ごとに再帰でたどる処理（clone、membership の登録、定数の判定など）が残る。そのため、既存の `deep_drop_expression_if_alternative`（10 万段の ELSE IF）は、検証を通った後にこれらの処理で stack を使い切りうる。v2 の構造の検証と metadata の解析で、式の木の根からの IR の深さも数え、128 を超えたら `Safety IR expression nesting exceeds 128` で拒否する。値は frontend の出力検査と同じ 128 で、frontend の出力と JSON・TS の decoder を通った IR はこの値を超えないため、どの経路の結果も変えない。上限の値は増やしていない。
- **期待値が変わる既存のテストに 1 件を足した。** `crates/rtlc/tests/deep_drop.rs` の `deep_drop_expression_if_alternative` は連鎖の形なので、checker 48 の代わりに上の IR の深さの拒否を期待する。ほかの deep_drop と手書き IR の 48 / 49 は連鎖の形ではなく、期待値を変えない。
- **既存のテストの分類。** 連鎖の形でないので期待値を変えないもの: rtlc の unit test（`v2_dynamic_call_depth_is_checked_before_exhausting_the_native_stack` の call の 127 / 128 / 129、`v2_enabled_subscripts_keep_the_depth_of_live_branch_steps`、action frame と ENABLED の frame の 48）、`crates/idealis-core/tests/deep_request_drop.rs` の action frame、`tests/tlc-differential/analysis-ir-corpus.ts` の `~` の 48 / 49、`tests/workspace-oracle/contract-test.ts` の snapshot の 48、deep_drop の連鎖でない 43 件。期待値を変えるもの: 上の 1 件、`crates/rtla-compiler/tests/nesting_chains.rs` の plan で宣言した 5 か所、`tests/nesting-chains/contract-test.ts` の同じ閾値と 54 枝の Quint の IF 連鎖の検査、`fixtures/nesting-chains/cases.json` の v2 の連鎖の綴りで checker 48 か動的評価 128 に拒否されていたセル。
- **#83 の分割。** `eval_operator` の 26 の腕と `eval_scalar_operator` の 12 の腕を、それぞれ別の関数に分けた。`eval` の `#[inline(always)]` は変えない。
- **置き場所。** rtlc の Red のテストは `crates/rtlc/tests/nesting_chains.rs`（IR を直接組む。共通の組み立ては `tests/chain_programs/`）、#83 は `crates/rtlc/tests/evaluation_stack.rs`（種類ごとの組み立ては `tests/stack_programs/`）。stack の harness は `evals/experiments/nesting-chains/stack-harness/`、0075 の入力は `evals/experiments/nesting-chains/sources-0075.ts`、期待値を導く規則は `expectations-0075.ts`。導いた期待値は `fixtures/nesting-chains/cases-0075.json` と `cases.json`、導くのに使った before と定数を外した before の複製の出力は `fixtures/nesting-chains/records-0075.json` に置く。
- **#83 の種類の大きさ。** 9 種類の入れ子は、定義の呼び出しで区切って checker 48 を避ける。1 定義あたり、式の演算子（`-`）・membership（`Seq`）・量化の束縛・ENABLED の step・binder の locals・時間論理（`[]`）は 30 段、値の演算子（`Cardinality({...})`）と遅延関数（`<<...>>[1]`）は 2 段の組を 15 個、call は 1 段とした。release の before で受理される最大の大きさと次の大きさは、テストを書く前に 1 GiB の thread で測った。
