# 0119: 定義を層状に重ねた call の評価の深さを、Wasm の上端の実測から 256 まで受理する

[#122](https://github.com/mizchi/idealis/issues/122)を扱う。[事前plan](../plans/0119-call-depth-limit.md)を 5674480c5 を基準に記録し、実装の段で変えた条件と、上限の値を 512 から 256 に変えた条件を、評価の結果を見る前に plan に追記した。評価の before は plan を記録した時点の origin/main（42b88c5a9）で、5674480c5 の後に入った Apalache の `MkSeq`（#121）の分だけ `crates/rtla-compiler/src` が異なる。push と remote merge は含めない。

rtlc の v2 で、再帰しない経路の動的評価の上限を 128 から 256 に上げ、並列検査の rayon の worker に 8 MiB の stack を指定した。再帰する定義に入った経路の上限 128（`bounded / evaluation`）、IR・wire・両 frontend・v1・要求の `bounds`・`contracts.ts`・Wasm の build の設定は変えていない。operator の定義を層状に重ねるだけの入力（連鎖を含まない）は、41 段で 128 に当たっていた。N 段の層は深さが 5 + 3N で、N = 83（深さ 254）までが complete になり、N = 84（深さ 257）以上は `Safety evaluation nesting exceeds 256` の型付きの拒否になる。

#122 の実モデルの深さ 232 は 256 に収まるが、差は 10.3% しかない。plan の記録にある観測用の複製の最大 264 の側は 256 では受理されない（下の「残る制約」）。plan が最初に選んだ 512 を採らなかった理由は、次の節にある。

## 値を 512 ではなく 256 にした理由

plan は、値の代わりを結果を見る前に決めていた。ある host の Wasm の上端が深さ 1,024 未満なら 512 を採らず、256 が同じ条件（上端が 512 以上、深さ 255 の最小の stack が release で 1,024 KiB 以下、debug で 6,144 KiB 以下）を満たすときだけ 256 を採る。

観測用の複製（上限 4,096、再帰の経路は 128）の Wasm を、要求ごとに新しい instance で動かし、13 種類と hop の受理される最大の大きさを二分探索して、その大きさでの評価の深さ（複製の記録）を読んだ。host ごとの最小は次のとおりで、どれも update の入れ子だった（[wasm-ceiling](../runs/0119-wasm-ceiling/summary.md)）。

| host                         | 最初の測定（値を決めた測定） | 最終の複製（`oracle-r2`） |
| ---------------------------- | ---------------------------: | ------------------------: |
| Node v24.22.3 既定           |                        1,237 |                     1,553 |
| Node `--liftoff-only`        |                        1,654 |                     1,697 |
| Node `--no-liftoff`          |                        1,237 |                     1,261 |
| Chromium の main thread      |                        1,559 |                     1,598 |
| Chromium の dedicated Worker |                      **806** |                   **827** |

Chromium の dedicated Worker だけが 1,024 を下回った（最初の測定では大きさ 777 まで受理され 778 で `RangeError: Maximum call stack size exceeded`）。Chromium は 2 回流して同じ値になった。このため 512 は採らない。256 は、上端の最小が 806 で 512 以上、深さ 250〜255 の最小の stack が release で最大 276 KiB（operator 245）、debug で最大 2,116 KiB（membership 239）なので、条件を満たす。232 との差が 10.3% しかない点が、この値の弱さである。512 のための実装・テストでの Green と stack の測定は、値を変える前に流した（[audit](../runs/0119-audit/) の `superseded`）。

## 採用した変更

- `EvalContext` と `Snapshot` の `evaluation_depth` を u16 にした（256 は u8 に入らない）。`descend` は、再帰する定義に入った経路で深さが 128 以上なら今と同じ `evaluation-budget`（`Recursive safety evaluation nesting exceeds 128`）、それ以外の経路で深さが 256 以上なら protocol の `Safety evaluation nesting exceeds 256` を返す。葉の評価の前の検査は低い方の 128 のままで、`descend` に判定を任せる。ENABLED の subscript の深さの見積もりも u16 で飽和させる。`size_of::<EvalContext>()` は before・after とも 72 byte。
- 並列検査の rayon の pool に 8 MiB の `stack_size` を指定した（stack の大きさは予約だけで、呼び出しの費用は増えない）。
- 変えた product code は `safety_expressions.rs`・`safety_lazy.rs`・`safety_parallel.rs` の 3 file（約 20 行）だけで、`crates/rtla/src`・`crates/rtla-compiler/src`・`crates/rtlc/src/ir.rs`・`packages/idealis/src`・`crates/idealis-core/src`・`adapters/go` の sha256 は before・after・評価の終了時で同じだった（[audit](../runs/0119-audit/checks/)）。
- 文書は `crates/rtlc/SUPPORT.md`、`crates/rtla-compiler/README.md`、`crates/rtla-compiler/SUPPORT.md`、`docs/rust-frontend.md`、`docs/safety-ir-v2.md`。

## テストと Red

実装の前にテストを置き、before（plan の commit にテストだけを足したもの）で意図した理由で失敗することを記録した（[audit](../runs/0119-audit/logs/)、`red*`）。

- `crates/rtlc/src/safety_definition_tests.rs`: call の 255 / 256 / 257 と ENABLED の subscript の拒否（8 MiB の thread）が before で失敗した（理由は `Safety evaluation nesting exceeds 128`）。再帰する定義の call の連鎖の 127 / 128 / 129 は before でも通る対照（23 件中 2 件が Red、21 件が通る）。
- `crates/rtlc/tests/evaluation_stack.rs`（13 種類の 256 の直前と直後、release は 2 MiB・debug は 8 MiB の thread）: debug と release の 13 件がすべて 128 の拒否で失敗した。after は debug・release とも 13 件が通る。大きさは観測用の複製の記録（受理される最大と次）で決め、最初に構造上の深さで決めた 4 種類（ENABLED、binder の locals、temporal、action）は、複製の記録を「上限と比べる値」に変えて直した（plan に追記）。
- `crates/rtlc/tests/parallel_stack.rs`（新）: 32 個の初期状態を持ち、Next の guard に membership の入れ子を置いたモデルを worker 2 で検査する。before で 2 件が 128 の拒否で失敗する。after は complete と型付きの拒否。このモデルは 2 MiB の worker の限界と深さ 255 が 1 段（約 8 KiB）で重なるので、pool の stack の Red には使えなかった。pool の stack の Red は `safety_parallel.rs` の unit test（worker の thread に 32 KiB の frame を 192 段積む）で取り、stack の指定がないと debug・release とも `has overflowed its stack`（SIGABRT）、指定すると通る。
- `crates/rtla-compiler/tests/call_depth.rs`（新、hop 40・41・76・83 が complete、84・85・168・169 と `chain_N50.tla` を含む 3 件）: before で 3 件が失敗した。
- `crates/rtla-compiler/tests/nesting_chains.rs` と `tests/nesting-chains/contract-test.ts`: `boundary-0075` の `calls-127`・`calls-128`・`calls-129` の 3 セルだけを `fixtures/call-depth/cases-0119.json` の同じ名前のセルで上書きする。before で 1 件と 3 件が失敗した。
- `tests/call-depth/contract-test.ts`（新、35 件）: before で 25 件が失敗した。after は 35 件が通る。`tests/wasm-host/contract.ts` と `tests/portable-browser-entry.ts`・`tests/portable.browser.spec.ts` の `runCallDepth`（N = 83 の complete、N = 84 の型付きの拒否、拒否の後の同じ instance での complete）は、Node の host で before の core が失敗し、after の core で通った。

期待値が変わった既存のテストは、plan が宣言したもの（上の unit test 2 件、`evaluation_stack.rs`、`calls-127〜129` の 3 セル）だけだった。実装の後に流した rtlc の 377 件と rtla-compiler の 259 件は、debug と release の両方で、ほかの失敗がなかった。

## 入力と期待値

`evals/experiments/call-depth/sources.ts` が 28 の入力を作り（`fixtures/call-depth/cases-0119.json` はこれに `calls-127〜129` を加えた 31 件）、期待値は `expectations-0119.ts` が導く。

- hop（wrapper つき、`step == counter' = hopN(counter) % 5`）と hop-invariant（TS frontend が `%` を持たないための綴り、`inv == hopN(counter) >= 0`）: N ∈ {40, 41, 76, 83, 84, 85, 168, 169}。固定した Quint の `chain_N50.tla`。
- 意味の対照: 最も内側の本文の `x \div 0`（N = 70）、評価されない枝の深い呼び出し 2 つ（N = 300）、`hop76(counter) < 80` の違反。
- 既知の限界と再帰の対照: `UNCHANGED Vn`（n = 6〜9）、`Loop`、`Sum(200)`、深さ約 155 の hop から再帰する定義を `Sum(x)` で呼ぶ入力。
- 期待値の導き方: before が再帰しない経路の `Safety evaluation nesting exceeds 128` で拒否した v2 のセルは、入力に宣言した深さが 256 以上なら `Safety evaluation nesting exceeds 256`、それ以外は観測用の複製の出力とする。それ以外のセルと v1 は before の出力。宣言した深さは観測用の複製の記録と一致する（[oracle](../runs/0119-oracle/)）。

plan の記述から変えた点: 定数引数の再帰呼び出し（`Sum(2)`）は評価の前に畳まれ、深さ 155 から呼んでも complete になったので、状態に依存する `Sum(x)` に変えた。`UNCHANGED V8` は深さ 256 に達し（V7 は 128）、256 では拒否される。512 なら受理される予測だった。

## 4 経路と before / after

378 入力（0074 の 265、0075 の 81、今回の 28 と Quint の 4）を、v1 / v2 と 4 経路（TS → Wasm、TS → native、Rust の Wasm、Rust の native）で before・after・観測用の複製に流した。各 3,024 セル（[paths](../runs/0119-paths/)）。

- before と after で、IR の sha256 が異なるセルは 0。TS と Rust の正規化した IR は、両方が compile できる 355 セルで一致した（before・after とも）。v1 の before と after の差は 0。
- v2 で before と変わったのは 72 セルで、すべて before が再帰しない経路の動的評価 128 で拒否していたセルだった。after での内訳は complete 36、`Safety evaluation nesting exceeds 256` 28、違反 4、`Integer division by zero` 2、`bounded / evaluation` 2。許す差の外の差は 0。
- after が観測用の複製と異なるのは、複製が complete にする入力を after が 256 で拒否する 28 セルだけ（hop 84 以上・hop-invariant 84 以上・`UNCHANGED V8`、work と文言を含めて複製の出力と照合した）。
- 同じ frontend の native と Wasm の結果が異なるセルは、before・after・複製とも同じ 5 セル（平たい 128・129 項と CASE 128 枝の v1 を Rust の Wasm が decoder で拒否する、0075 の記録と同じ）だけ。after の結果は、この 5 セルを除いて記録した期待値と一致した。

## TLC との比較

固定 TLC 1.7.4（jar の sha256 936a2620…、`-Xmx256m -Xss8m -workers 1`、ケースごとに scratch と metadir を分けた）を新しい 28 入力に 1 つずつ流した（[TLC](../runs/0119-tlc/)）。`calls-127〜129` は入力が 0075 と同じなので、[0075-tlc](../runs/0075-tlc/) の verdict（complete 6 / 7）を引用した。`chain_N50.tla` の TLC には Apalache 0.63.0 の module を classpath に置いた。

- TLC の verdict: complete 25、0 除算 1、違反 1（trace 4）、`Loop` の Java の StackOverflowError 1。
- after の Rust の native の v2 は、15 入力で TLC と状態数・生成数・違反の trace の長さ・エラーの種類が一致した（before は 5）。残りは各層の上限による型付きの拒否（動的評価 256 が 9、frontend の leaf 256 が 1）と、再帰の `bounded / evaluation` 3 件（TLC は `Sum(200)` と、深さ約 155 から呼ぶ入力で complete、`Loop` は StackOverflowError）。N = 83 は complete 5 / 6（work 862）で、N = 84 は 256 で拒否される。
- TS frontend の v2 は 9 入力で一致した。ほかは TS が `%`・`\div`・EXTENDS・RECURSIVE を持たないことによる拒否と、上の上限。
- v1 の差は before と after で 0。v1 が再帰を持たない（compile で拒否する）ことによる不一致 8 行は、before・after とも同じ。
- 0074・0075 の入力で before と after が変わった v2 のセル（calls-127〜129 の Rust 3・TS 3）は、引用した TLC の verdict と一致した。

## stack

0075 の harness と同じ形の `evals/experiments/call-depth/stack-harness/` を before・after・観測用の複製の debug と release で build し、`check` だけを指定した大きさの thread で流して、1 GiB と同じ出力になる最小の stack を 4 KiB の分解能で二分探索した（[stack](../runs/0119-stack/)、rustc 1.100.0、aarch64-apple-darwin）。336 セルのすべてで、最小より 4 KiB 小さい stack は `has overflowed its stack` で abort した。

深さ 250〜255 で受理される 13 種類の最小の stack（単位は KiB）の最大は、release が 276（operator 245）、debug が 2,116（membership 239）で、plan の条件（release 1,024 以下、debug 6,144 以下）を満たした。after の最小は、深さ 127 以下で受理されるセルでは before と同じ（release の membership 119 が 100 から 116、operator 120 が 148 から 132 に変わった以外）。観測用の複製の最小も同じ値になった。hop は N = 83（深さ 254）で release 148、debug 1,076。debug の 2 MiB の thread は、membership が深さ約 250 で 2,116 KiB を使うので、上限の手前で足りなくなりうる（要件は 8 MiB）。

## Wasm

after の core を、観測用の複製と同じ host（Node の既定・`--liftoff-only`・`--no-liftoff`、Chromium の main thread と dedicated Worker）で、要求ごとに新しい instance で動かした（[wasm-ceiling](../runs/0119-wasm-ceiling/)）。5 host × 15 セル（13 種類、hop の IR と `check-tla`）で、受理される最大の大きさが深さ 255 の大きさ（hop は N = 83）と一致し、次の大きさは型付きの `Safety evaluation nesting exceeds 256` で拒否された。`RuntimeError` と `RangeError` は 0 件。拒否の後も同じ instance で次の要求が complete になることを、Node の host と Chromium の `runCallDepth` で確かめた。Go の adapter（wazero v1.12.0、go@1.25.14）は、観測用の複製で hop 83 / 84 / 168 / 169 が complete、1,017 が complete、1,018 が trap（線形メモリの stack）、after で 83 が complete、84 以上が型付きの拒否だった。

CLI（wasm32-wasip1、`node:wasi`）は測れなかった。`idealis` は IR の version を選べず v1 を使い、v1 の frontend は hop を 17 段から `expression or operator nesting exceeds 40` で拒否するので、v2 の評価の上限に届かない（before・after・複製とも同じ）。

## 既存の入力と完全応答

- `evaluator-corpus`（Quint6・TCommit・共有 chain、8 経路・各 4 iteration）の 264 比較が、program・結果・work を含めて一致した（[corpus](../runs/0119-corpus/)、[summary](../runs/0119-corpus-summary/)）。
- `temporal-parity` の旧版 156 モデル・1,560 要求が、before・after の native・Wasm で一致した（[temporal](../runs/0119-temporal-parity/)）。

## Criterion

0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で、14 関数の kernel を流した（[kernel](../runs/0119-kernel-summary/)）。after / before の比は 0.974〜1.011 で、10% 以上の後退は 0 件だったので、ABBA の再確認は流していない。frontend を変えていないので、rtla-compiler の bench は流していない。

## 検査

- `just check`（書式、型、vitest 79 件、node:test 79 件）、`just wasm-test`（24 件）、`just safety-test`（1,784 件、新しい contract を含む）、`just quint-test`（38 件）、`just portable-build` と `just portable-test`（Rust 718 件、node:test 3・22・9 件）、`just go-test`（CGO 無効と race の 2 回、vet、consumer）。
- stack のテストは debug（`cargo test`）と release（`cargo test --release`）の両方で通った（rtlc 377 件、rtla-compiler 259 件。release の stack 3 test target も）。
- `just browser`（Chromium 7 件）と `just portable-browser`（8 件、`runCallDepth` を含む）。
- `just mutation`（スコア 95.4%、生き残り 5 件は `examples/durable-counter.ts`。変異の対象のファイルと、そのテストは変えていない）。
- `just runtime-build` と `just runtime-test`、`call-depth-harness`（harness の cargo fmt と clippy `-D warnings`）。
- `just evaluate` と `just replay-evaluation`、`just eval-loop` と `just replay-loop`（[loop](../runs/0119-loop-development/)）。
- `just support-check`。

実モデル（Quint で書かれたもの）は、この評価では流していない。plan に記録された、観測用の複製で測った 31 件の評価の深さの最大は 264 と 151 で、256 では 264 の側が `Safety evaluation nesting exceeds 256` で拒否され、151 の側は受理される（31 件中 17 件が `maxWork` で止まるので、どちらも探索した範囲での下限）。この記録は採否の条件に含めない。

記録した run では、実行したホストの絶対パスとユーザー名を、記録する時点で placeholder に置き換えた。ケースごとの出力と log は消した。run の経緯と、やり直した run は [audit](../runs/0119-audit/) に残した。

## 残る制約

- 再帰する定義に入った経路は深さ 128 で `bounded / evaluation` になり、TLC が受理する深い再帰は未確定のまま。
- 深さ 256 以上の入力は型付きで拒否する。plan の記録にある実モデルの 264 は 256 に収まらず、232 との差も 10.3% しかない。Wasm の Chromium の dedicated Worker は update の入れ子で深さ約 800 に届く程度の stack しか持たないので、512 は host の余裕の条件を満たさなかった。上限を上げるには、評価の再帰を減らす候補（plan の候補 2: 引数が `Parameter` / `Capture` だけの転送を loop で解決する、候補 1: `Update` と `At` の spine を反復でたどる）が先になる。
- `UNCHANGED V8`（leaf 256 個）は v1 が受理し、v2 は 256 で拒否する。
- debug の build を 2 MiB の thread で動かす利用者は、深さが 250 近くになると abort しうる（要件は 8 MiB）。
- CLI の Wasm では v2 の評価の上限を測れていない。

## レビューへの対応

CodeRabbit の指摘（[#125](https://github.com/mizchi/idealis/pull/125)）を、コードと記録で確かめて次のとおり扱った。

- **ENABLED の subscript の context が再帰する定義の印を落とす（Minor）: 有効。** subscript の context を `EvalContext::new` から作っていたので `recursive` が `false` になり、再帰する定義の中の ENABLED では、深さ 128 の subscript が 256 までの上限で評価されて complete になった（再帰する経路は 128 で `bounded / evaluation`）。再現: 再帰の印を付けた引数なしの定義の本文に、step が 124 個の ENABLED（subscript は定数）を置く。subscript の深さがちょうど 128 になる。回帰 `v2_enabled_subscripts_keep_the_recursive_limit`（再帰の step 123 は complete、再帰の step 124 は `bounded / evaluation`、再帰でない step 124 は complete）は、修正前に再帰の step 124 で `left: "complete"`・`right: "bounded"` と食い違って失敗した。直し方は、subscript の context に `recursive: ctx.recursive` を足した 1 行。再帰でない経路の 256 は変わらない。記録した 378 入力のうち ENABLED を持つ 7 入力は再帰を宣言せず、記録した結果は変わらない。
- **遅延関数の適用が呼び出し側の深さを数え直す（Major）: 再現しない。** `FunctionValue::Lazy` は `eval_function` の `Function` の腕だけが作り、`eval_at`・`eval_domain`・`At` の腕が同じ評価の中で `at` / `select` / `domain` に渡して消える。評価をまたいで残る経路はない。作る時の context の深さは、適用する側の context から `descend` で進んだ値で、減る経路がない。引数は使う場所で評価される（`Parameter` は `Argument::evaluate` が使う側の context で式を評価する）ので、関数式を呼び出しの上流に書いても、snapshot の深さは使う場所の深さになる。確認の test `v2_applied_lazy_function_body_counts_the_depth_of_its_application` は、関数式を最上位に置き、70 段の引数の転送の先で適用し、本文に 130 段の call の連鎖を置く。転送だけ（70 段）と本文だけ（130 段）は complete で、合わせると `Safety evaluation nesting exceeds 256`（protocol）で拒否される。この test は修正前の head で通り、product code は変えていない。対照として、`Snapshot::new` が深さを 0 で記録する一時的な変更を入れると、この test は拒否される場所で complete になって失敗した（変更は戻した）。
- **文書の 128（Minor、review 本文）: 有効。** `docs/safety-ir-v2.md` と `crates/rtlc/SUPPORT.md` の動的評価の上限の記述を、再帰しない経路は 256、再帰する定義に入った経路は 128 に直した。
- **`analyze-paths.py` が、変わった再帰の `bounded` の結果を許す差に分ける（Minor）: 記録した結果は影響を受けていない。** 分類の式は、before が `bounded` で after が result なら `allowed` と書くので、script の欠陥は実在する。run の記録は編集しないので、記録から確かめた。`0119-paths/differences.tsv` は、v2 の全セル（378 入力 × 4 経路）のうち before と after の結果（status・件数・work を含む）が異なるものを全て書き出している。72 行すべての before が `Safety evaluation nesting exceeds 128` の `check` で、before が `bounded` の行は 0 だった（確認: `python3 -I` で `differences.tsv` を読み、72 行中 before の `status` が `bounded` の行を数えた）。固定入力で v2 の期待が `bounded / evaluation` の 7 入力（Rust の frontend、native と Wasm で 14 セル）のうち、6 入力（`recursive-plain-from-40`、`recursive-if-54-from-40`、`recursive-plain-from-60`、`recursive-if-54-from-60`、`recursion-loop`、`recursion-sum-200`）は `differences.tsv` に現れず、before と after が work まで同じである。残りの `recursion-from-hop-50` の 2 セルは before が非再帰の 128 の拒否で、after は観測用の複製と同じ `bounded / evaluation`（work 130）だった。`analysis.txt` の「期待との不一致 5」は、v1 の `flat-and-128`・`flat-or-128`・`flat-and-129`・`flat-or-129`・`case-other-128` を Rust の Wasm が decoder で拒否する 5 セルで、再帰とは関係しない。before・after の生の json は `reduced.json` のとおり消してあるので、この確認は `differences.tsv` と固定入力の期待に依る。
- **ほかの driver script（終了状態、上書き、`record.py` の置換、`stage256.sh` の再 build）: script は記録の一部なので編集せず、記録した結果が変わっていないことだけを確かめた。** 各 stage の command は `run.sh` が 1 件ずつ `commands.tsv` と `logs/*.exit` に自分の終了状態を書くので、外側の script が 0 を返しても個々の失敗は隠れない。`stage256.sh`・`loops.sh`・`final2.sh`・`master.sh` が呼ぶ command は、意図した Red と、audit の README が書くやり直し前の行（`green-ts-call-depth` の 1 回目）を除いて、すべて `exit=0` だった。`stage256.sh` の `after` の再 build（`stack-build-after-*-r7`）も、debug・release とも `exit=0` で、release は実際に再 build されている（24 秒）。`record.py` の裸のユーザー名の置換は、0119 の run 全体で `0119-corpus/manifest.json` の Java の property `user.name` の 1 件だけに当たった。`0119-audit` は commit が 1 つ（cca78dc11）だけで、`assemble-audit.sh` の `rm -rf` が消した commit 済みの記録はない。`summarize-stack.py` と `summarize-wasm.py` を、commit した測定の json から再実行すると、commit した `summary.md` とバイト単位で一致した。
- **`cli-ceiling.ts` の `operator` の不変条件が偶数の大きさで偽になる（Minor）: 有効。** 偶数の大きさでは `negN(counter)` が `counter` と等しく、`counter` が 1 になると `Inv` が破れる。このため `cli-after.json` と `cli-oracle.json` の `operator` の行（`largest: 7`、`Invariant Inv is violated`）は、入れ子の上限ではなく違反を測っている。CLI についての結論（v1 しか出さないので v2 の上限に届かない）は `hop` の行だけに依り、`operator` の行は使っていないので変わらない。不変条件を `<= 4`（`counter` は 0〜4 なので、どの大きさでも成り立つ）に直した。`rtlc` の `safety_expressions.rs` が前の Wasm の build より後に変わっていたので、CLI の Wasm も build し直し、新しい run [0119-cli-r2](../runs/0119-cli-r2/) で after だけ測り直した（範囲は `slot-f.sh` の after と同じ `hop:1:300 operator:1:300`）。`hop` は 16 まで complete で、17 が `Unsupported TLA input: expression or operator nesting exceeds 40`（exit 150）。前と同じ。`operator` は 17 まで complete で、18 が同じ拒否（exit 150）になり、`Invariant Inv is violated` は出なかった。前の run `0119-wasm-ceiling` の `cli-after.json` と `cli-oracle.json` は編集せず、`operator` の行は違反を測った値として残す。観測用の複製の側（`cli-oracle.json`）は測り直していない。CLI の結論は変わらない: 両セルとも v1 の frontend の上限 40 で拒否され、v2 の 256 の手前にある。CLI は v1 しか出さないので、v2 の上限には届かない。
