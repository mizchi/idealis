# 0119: 定義を層状に重ねた call の評価の深さを、stack の実測から 512 まで受理する

[#122](https://github.com/mizchi/idealis/issues/122)を扱う。基準は 5674480c5（origin/main）。[0075](../iterations/0075-nesting-chains-ir-eval.md)は線形の連鎖を 1 段と数え、評価の再帰の frame を debug で小さくした（[#83](https://github.com/mizchi/idealis/issues/83)）。連鎖を含まない入力でも、operator の定義を層状に重ねるだけで、rtlc の v2 の動的評価の上限 128 に当たる。この iteration では rtlc の v2 の評価だけを変え、再帰しない経路の動的評価の上限を 128 から 512 に上げる。値は 1 段あたりの stack の実測から決め、native の worker pool の stack を明示する。再帰の経路の上限 128（`bounded / evaluation`）、IR・wire・両 frontend、v1 の数え方と結果、要求の `bounds`、`contracts.ts`、ほかの上限（frontend 40、checker の式 48、IR の深さ 128、frontend の出力検査 128、JSON と TS の decoder 128、parser 64、action frame 48）、Wasm の build の設定は変えない。push と remote merge は含めない。

## 観測

5674480c5 で測った（rustc 1.100.0、aarch64-apple-darwin、Node v24.22.3）。入力は #122 の公開用の合成モデルと、0075 の `crates/rtlc/tests/stack_programs/` の 9 種類の入れ子だけを使う。Quint で書かれた実モデルは使わない。

再現は N 段の層である。`hopK(x) == hop(K-1)(x) + 1` の定義を重ね、`step == counter' = hopN(counter) % 5` から呼ぶ。固定版 Quint 0.33.1 の生成物 `evals/experiments/call-depth/quint/chain_N50.tla`（sha256 33837cba114decbd6d5a42ba502661fb2365e06694ff380b49a5c98dd02a12d6、Quint の生成器は同じ場所の `gen.py`、sha256 670ec8ac7a2905efcfd52959a8105b49438695e0bb85646f1f1d8b977db93987）と、同じ形を手で書いた TLA+（`EXTENDS Integers`、`init`・`inv`・`step` と、`q_init`・`q_step`・`q_inv` の wrapper）は、下の観測用の複製で同じ結果になった（complete 5 状態 / 6 生成 / work 532、評価の深さの最大 155）。

before（5674480c5、release、Rust の API、1 GiB の thread）の結果。

| 入力（wrapper つき） | 結果                                    |
| -------------------- | --------------------------------------- |
| N = 40               | complete 5 / 6、work 432                |
| N = 41               | `Safety evaluation nesting exceeds 128` |
| N = 168              | 同上                                    |
| `chain_N50.tla`      | 同上                                    |

観測用の複製（5674480c5 の rtlc で、再帰しない経路の上限を 4096 にし、`evaluation_depth` を u16 にして、`descend` の後の深さの最大を記録したもの。観測専用で、この評価の外では使わない）で、同じ入力を検査した。

| N   | 評価の深さの最大 | 結果                       |
| --- | ---------------: | -------------------------- |
| 40  |              125 | complete 5 / 6、work 432   |
| 41  |              128 | complete 5 / 6、work 442   |
| 76  |              233 | complete 5 / 6、work 792   |
| 84  |              257 | complete 1 / 2、work 176   |
| 85  |              260 | complete 5 / 6、work 882   |
| 168 |              509 | complete 5 / 6、work 1,712 |
| 169 |              512 | complete 1 / 2、work 346   |

- 深さの最大は 5 + 3N で、#122 の記録（call、本文の `+`、`Argument::evaluate` による `Parameter` の転送の 3 段）と一致する。上限 L は、深さ L に達する入力を拒否する（N = 41 の深さ 128 が今の最初の拒否）。
- work は 10N + 32 になる。`hopN(0) = N + 1` なので、N + 1 が 5 の倍数のとき（N = 84、169）は `counter' = 0` で止まり、1 状態 / 2 生成になる。これは入力の意味で、上限とは関係しない。

### native の stack

0075 の harness（`evals/experiments/nesting-chains/stack-harness/`）と同じ形の harness で測った。release は opt-level 3・LTO・codegen-units 1・panic=abort、debug は既定に panic=abort を加え、`CARGO_INCREMENTAL=0` で build した。parse・compile と program の組み立ては 1 GiB の thread で行い、`check` だけを指定した大きさの thread で動かす。1 GiB と同じ出力になる最小の stack を、4 KiB の分解能で二分探索した。最小より 4 KiB 小さい stack では、どのセルも `thread '<unknown>' has overflowed its stack` で abort した（終了コード -6）。hop は wrapper のない綴り（work は 10N + 27、深さは同じ）。単位は KiB。

| 入れ子（大きさ）              | 評価の深さの最大      | release              | debug                   |
| ----------------------------- | --------------------- | -------------------- | ----------------------- |
| hop（40 / 80 / 160 / 320）    | 125 / 245 / 485 / 965 | 68 / 148 / 308 / 612 | 532 / 1,044 / 2,052 / – |
| operator（120 / 240 / 480）   | 125 / 249 / 497       | 132 / 276 / 548      | 660 / 1,268 / 2,516     |
| value（60 / 120 / 240）       | 125 / 249 / 497       | 52 / 132 / 260       | 724 / 1,428 / 2,804     |
| call（120 / 240 / 480）       | 120 / 240 / 480       | 36 / 84 / 180        | 404 / 772 / 1,508       |
| membership（120 / 240 / 480） | 127 / 251 / 499       | 116 / 228 / 468      | 1,188 / 2,132 / 4,036   |
| binding（120 / 240 / 480）    | 132 / 264 / 528       | 52 / 116 / 228       | 292 / 548 / 1,076       |
| enabled（120 / 240 / 480）    | 129 / 257 / 513       | 68 / 148 / 292       | 948 / 1,860 / 3,684     |
| bind-run（120 / 240 / 480）   | 133 / 265 / 529       | 52 / 116 / 260       | 356 / 676 / 1,316       |
| lazy（60 / 120 / 240）        | 125 / 249 / 497       | 52 / 116 / 244       | 964 / 1,892 / 3,764     |
| temporal（120 / 240）         | 125 / 249             | 36 / 84              | 580 / –                 |

temporal の 240 は work 1,000,000 で `bounded / work` になる（1 GiB の出力も同じ）。

1 段あたりの増分（大きさの 2 番目と 3 番目の差を深さの差で割った値）は、release で最大 1.10 KiB（operator）、membership 0.97、hop 0.63〜0.67、ほかは 0.39〜0.56。debug で最大 7.7 KiB（membership）、lazy 7.5、enabled 7.1、value 5.5、operator 5.0、hop 4.2、call 3.1、bind-run 2.4、binding 2.0。#83 の release 約 0.63 KB と hop は同じ値になった。外挿すると、深さ 511 の最小の stack は release で最大約 560 KiB（operator）、debug で最大約 4,130 KiB（membership）になる。

- release の 2 MiB の thread では、N = 168（深さ 509）が complete 5 / 6 になった。operator の 500（深さ 518）も complete。
- debug の 2 MiB の thread では、N = 168 が `fatal runtime error: stack overflow, aborting` で abort した（終了コード 134）。N = 76（深さ 233）は complete。membership は深さ 251 で 2,132 KiB を使うので、debug の 2 MiB は membership の約 240 段までしか持たない。
- debug の 8 MiB の thread では、N = 168 と membership の 500（深さ 520）が complete。

### Wasm の stack

`idealis_core.wasm`（wasm32-unknown-unknown、release）は、`__stack_pointer` の初期値と最初の data segment の offset がどちらも 1,048,576 で、1 MiB の線形メモリの stack を先頭に置く。観測用の複製の core を Node v24.22.3 の既定の stack で、要求ごとに新しい instance で動かし、受理される最大の大きさを二分探索した。program は native の harness が組んだ IR を `check-safety` で渡した（hop は `check-tla` でも同じ値）。括弧の中は、同じ IR を native の複製で検査したときの評価の深さの最大。

| 入れ子     | `--liftoff-only`         | 既定                 | `--no-liftoff` | 次の大きさでの失敗                             |
| ---------- | ------------------------ | -------------------- | -------------- | ---------------------------------------------- |
| hop        | 1,017（3,056）           | 1,017                | 1,017          | `RuntimeError: memory access out of bounds`    |
| operator   | 1,787（1,848）           | 1,787                | 1,787          | `RuntimeError: memory access out of bounds`    |
| value      | 1,278（2,643）           | 1,307                | 1,306          | `RangeError: Maximum call stack size exceeded` |
| bind-run   | 2,351（2,589）           | 1,894 以下（不安定） | 1,950          | 同上                                           |
| membership | 3,124（3,232）           | –                    | –              | 同上                                           |
| binding    | 2,950（3,247）           | –                    | –              | 同上                                           |
| enabled    | 3,378（3,605）           | –                    | –              | 同上                                           |
| lazy       | 1,855（3,835）           | –                    | –              | 同上                                           |
| temporal   | 3,713（3,838）           | –                    | –              | 同上（受理側は `bounded / work`）              |
| call       | 3,900 以上（3,900 以上） | –                    | –              | 探索の上端まで失敗しなかった                   |

- 最も浅い失敗は operator の深さ約 1,849 で、線形メモリの stack（1 MiB）が先に尽きた。ほかの種類は engine の stack が先に尽き、深さ約 2,090（bind-run の既定、2,351 で 2,589 の比から）以上だった。
- bind-run の既定は、二分探索の後に 1,894 を流し直しても `RangeError` になった。tier の切り替えで engine の stack の使い方が変わり、境界は一定しない。
- どちらの失敗も型付きの拒否ではない。0048 の記録のとおり、失敗した後の instance は信頼できない。

### 上限の場所と、上限に依存するもの

- 上限: `crates/rtlc/src/safety_expressions.rs:113-129`（`EvalContext::descend`、再帰の経路は `evaluation-budget` の `Recursive safety evaluation nesting exceeds 128`、それ以外は protocol の `Safety evaluation nesting exceeds 128`）、同 814（葉の評価の前の検査）、同 25（`evaluation_depth: u8`）、同 1217-1218（ENABLED の subscript の深さの見積もり、u8 で飽和）、`crates/rtlc/src/safety_lazy.rs:14`（`Snapshot` の u8）。[0075-oracle](../runs/0075-oracle/)の `oracle.patch` は、再帰しない経路の 128 を外し、再帰の経路の 128 を残す複製をすでに使っている。
- stack の持ち主: native の `idealis-core` binary は main thread で `Engine::dispatch` を動かす（`crates/idealis-core/src/main.rs`、macOS と多くの Linux の既定の `ulimit -s` で 8 MiB）。並列の検査は rayon の pool の thread で評価する（`crates/rtlc/src/safety_parallel.rs:71`、既定 2 MiB）。並列は frontier が 16 状態以上で、`is_expensive` が真のときだけ使う（`crates/rtlc/src/safety.rs:574-592`）。Rust のテストの thread は既定で 2 MiB。Wasm の core は host の import を持たない（`crates/idealis-core/src/wasm.rs`）。CLI は wasm32-wasip1 で build し（`just cli-wasm-build`）、`node:wasi` で動かす（`packages/idealis/cli/tla-tools.ts`）。Go adapter は同じ core を wazero v1.12.0 で動かす。Playwright の project は Chromium だけ。
- 再帰しない経路で 128 に当たることを固定しているもの: rtlc の unit test `v2_dynamic_call_depth_is_checked_before_exhausting_the_native_stack`（127 / 128 / 129）と `v2_enabled_subscripts_keep_the_depth_of_live_branch_steps`（`safety_definition_tests.rs`）、`crates/rtlc/tests/evaluation_stack.rs`（9 種類の 128 の直前と直後、debug の 2 MiB）、`fixtures/nesting-chains/cases-0075.json` の `calls-127`・`calls-128`・`calls-129` の v2（`crates/rtla-compiler/tests/nesting_chains.rs` と `tests/nesting-chains/contract-test.ts` が読む）。文書は `crates/rtlc/SUPPORT.md`（85、87）、`crates/rtla-compiler/README.md`（15、17）、`crates/rtla-compiler/SUPPORT.md`（66 の `UNCHANGED Vn` の n = 7 の拒否、80）、`docs/rust-frontend.md`（60）、`docs/safety-ir-v2.md`（67）。
- 再帰の経路の 128 を固定しているもの（この変更で変えない）: `crates/rtla-compiler/tests/recursive.rs`、`recursive_actions.rs`、`crates/rtlc/tests/recursive_actions.rs`、`dead_bindings.rs`（`deep-action-21`）、`fold_order.rs`、`nesting_chains.rs`、`tests/frontend-semantics/`、`tests/recursive-actions/`、`tests/tla-coverage/`、`tests/tlc-differential/operator-sharing-test.ts`、`tests/wasm-host/contract.ts` と `tests/portable.browser.spec.ts` の `evaluation`。文書は `docs/recursive-actions.md`（33）、`docs/safety-ir-v2.md`（116）、`docs/rust-frontend.md`（58）、`crates/rtla-compiler/SUPPORT.md`（98）。

## 候補

簡単な順に並べる。どれも、TLC に揃えた評価順・短絡・最初に報告するエラー・work を変えないことを前提にする。

1. **`Update` と `At` の spine を #94 と同じく反復でたどり、1 段と数える。** 再現の hop には spine がないので、再現の深さは変わらない。#122 の記録のとおり、Quint で書かれた実モデルの深さ 232 も 128 に収まらない。採らない。plan を記録する前に、再帰しない経路の上限を 4,096 にした観測用の複製で、手元の 2 つの実モデルが保存していた 31 件の検査要求を流した。評価の深さの最大は 264 と 151 だった。どちらも 128 を超え、512 には収まる。ただし 31 件中 17 件は、`maxWork` の上限で `bounded`（`work`）として止まった。そのため、この最大値は探索した状態の範囲での下限である。
2. **引数が `Parameter` / `Capture` だけの転送を loop で解決し、hop ごとに `arena.consume(1)` を残す。** hop の 1 段が 3 段から 2 段になり、上限 128 のままでは N = 61 まで。深さ 232 には届かない。採らない。512 を超える入力が出たときの次の候補として残す（評価の再帰も 1 段分減る）。
3. **上限を実測から決め直す（採る）。** 再帰しない経路の上限を 512 にし、`evaluation_depth` を u16 にする。再帰の経路の 128 は変えない。rayon の pool の thread の stack を明示する。値の比較:
   - 256 は 232 の 10% 上で余裕がない。debug の 2 MiB は membership の約 240 段で尽きるので、256 でも debug の thread の要件は変わる。512 にしても、release と Wasm の余裕は十分に残る（下の数）。
   - 512: release の深さ 511 は最大約 560 KiB で、2 MiB の thread に約 3.6 倍の余裕がある。debug は最大約 4.1 MiB で、8 MiB の thread に約 2 倍の余裕がある。Wasm の最も浅い失敗（深さ約 1,849）に約 3.6 倍の余裕がある。
   - 1,024: Wasm の余裕が約 1.8 倍になり、debug は約 7.9 MiB で 8 MiB の main thread にほとんど余裕がない。採らない。
4. **上限を要求の `bounds` で指定できるようにする。** 安全な上限を決めるのは host の stack で、Wasm の線形メモリの stack と engine の stack は要求から大きくできない。指定できる値も測った上限で頭打ちにすることになり、固定の値と同じになる。その一方で、`SafetyBounds` の field を足すと、Rust の struct literal 約 170 か所、`contracts.ts`、TS decoder、Go adapter が変わる。既定を 128 のまま残すと、この issue は既定では直らない。採らない。
5. **`check` ごとに、大きさを指定した thread で評価する（0075 の c）。** release の深さ 511 は既定の 2 MiB の thread に収まるので、要らない。prepared な小さい問い合わせ（[0064](../iterations/0064-prepared-safety.md)）に呼び出しごとの thread の費用が足される理由も 0075 と同じ。採らない。stack を指定するのは、すでに検査ごとに作っている rayon の pool だけにする（stack の大きさは予約だけで、呼び出しの費用は増えない）。
6. **build の profile ごとに上限を変える。** 0075 と同じ理由（同じ入力の結果が debug と release で変わる）で採らない。
7. **再帰の経路の 128 も上げる。** 停止しない再帰の `bounded / evaluation` の work が、既存のすべてのテストで変わる。debug では、action の再帰（約 7 KiB / 段）が 2 MiB の thread を超える。この issue の再現と、#122 が挙げた実モデルの深さの経路（call、引数の転送、EXCEPT、`At`、action の step）は再帰を含まない。採らず、別の iteration の候補にする。
8. **call と本文の評価を、明示的な heap 上の stack に移す。** 評価器の書き直しになる。対象外。

## 仮説

rtlc の v2 で、再帰しない経路の動的評価の上限を 128 から 512 に上げ、再帰の経路の 128 を残せば、再現の N = 168（深さ 509）は固定 TLC と同じ 5 状態 / 6 生成で complete になり、N = 169（深さ 512）は `Safety evaluation nesting exceeds 512` の型付きの拒否になる。この拒否は、release の 2 MiB 以上の thread、debug の 8 MiB 以上の thread、rayon の pool（8 MiB を指定する）、既定の 1 MiB の線形メモリの stack を持つ Wasm（Node 24 と Chromium）で、stack が尽きる前に返る。評価順・短絡・最初のエラー・work は変わらず、今受理される入力と `bounded` になる入力の応答は byte 単位で同じになる。

- `descend` は、再帰の経路で深さが 128 以上なら今と同じ `evaluation-budget`（`Recursive safety evaluation nesting exceeds 128`）を返す。それ以外の経路で深さが 512 以上なら protocol の `Safety evaluation nesting exceeds 512` を返す。v1 で早く戻る条件（metadata・定義・frame のどれもない program）は変えない。
- `eval` の葉の前の検査は、低い方の 128 のまま `descend` に判定を任せる。再帰の経路の葉の扱いは今と同じになる。
- `evaluation_depth` は `EvalContext` と `Snapshot` で u16 にする。ENABLED の subscript の見積もりも u16 で飽和させる。64 bit の target で `size_of::<EvalContext>()` は変わらない見込み（3 つの slice、`Option<Scalar>`、`Option<&CallFrame>` の後に、bool 2 個と u16 が padding に入る）。
- rayon の `ThreadPoolBuilder` に 8 MiB の `stack_size` を足す。値は、debug の深さ 511 の最大約 4.1 MiB に約 2 倍の余裕を持たせたもの。profile で分けない。
- Wasm の linker の stack、`idealis-core` と CLI の `main`、要求の `bounds` は変えない。

予測（結果を見る前に固定する）。

- Rust の API と 4 経路（Rust frontend の native / Wasm、TS frontend から native / Wasm）の v2: N ∈ {41, 76, 85, 168} は complete 5 / 6、work 442、792、882、1,712。`chain_N50.tla` は complete 5 / 6、work 532。N = 169 は検査時の `Safety evaluation nesting exceeds 512`。N = 84 は complete 1 / 2、work 176。v1 は 0075 の記録と同じ層（frontend の 40）で、before と同じ。
- 0075 の `calls-127`・`calls-128`・`calls-129` の v2 は complete 6 / 7 になる（work は観測用の複製と同じ）。
- `UNCHANGED Vn` の v2 は、n = 7 と n = 8 で v1 と同じく受理され、n = 9 は今と同じ frontend の leaf 256 の上限で拒否される。
- stack: どの種類も、深さ 511 の最小の stack が release で 1,024 KiB 以下、debug で 6,144 KiB 以下になる（予測は最大約 560 KiB と約 4,130 KiB）。release の 2 MiB と debug の 8 MiB の thread で、深さ 511 は受理、512 は型付きの拒否になる。
- Wasm: 観測用の複製の受理の上端は、どの種類も深さ 1,024（上限の 2 倍）以上になる。after は深さ 511 を受理し、512 を型付きで拒否する。拒否の後も同じ instance で次の要求が complete になる。
- Criterion に再現する 10% 以上の後退はない。

## 固定条件と対照

- before は 5674480c5 の source / binary。after は before にこの変更だけを加えたもの。それぞれの sha256 を記録する。`crates/rtla/src`、`crates/rtla-compiler/src`、`crates/rtlc/src/ir.rs`、`packages/idealis/src`、`crates/idealis-core/src`、`adapters/go` が before・after・評価の終了時で変わらないことを sha256 で確かめる。変わる product code は `crates/rtlc/src/safety_expressions.rs`、`safety_lazy.rs`、`safety_parallel.rs` だけとする。
- 観測用の複製（`0119-oracle`）は 0075-oracle と同じ作り方にする。5674480c5 の rtlc で、再帰しない経路の 128 だけを外し（再帰の経路の 128 は残す）、`evaluation_depth` を u16 にして、深さの最大を読める関数を足す。patch と binary の sha256 を保存し、この評価の外では使わない。
- テストを先に置き、before で意図した理由で失敗すること（Red）を記録する。stack overflow は test binary 全体を abort させるので、stack のテストは別の test target にし、abort をそのテストの失敗として特定できるようにする。
  - `crates/rtlc/tests/stack_programs/mod.rs` に種類を足す: `forward`（`Parameter` の転送の連鎖。call の引数が呼び出し側の `Parameter`）、`update`（`EXCEPT` の target の入れ子）、`function`（`[k \in {1} |-> inner][1]` の遅延関数）、`action`（Next の branch の step の連続）。temporal は、`[]` を重ねると深さ 511 で work が予算を超えるので、1 段ごとに temporal の call を重ねる形にする。どの種類も、深さ 511 で受理され 512 で拒否される大きさを、テストを書く前に観測用の複製で決める（0075 と同じ）。
  - `crates/rtlc/tests/evaluation_stack.rs`: 13 種類の 512 の直前と直後を、release は 2 MiB、debug は 8 MiB の thread で検査する。before は直前の入力が 128 で拒否されて失敗する。
  - `crates/rtlc/tests/parallel_stack.rs`（新しい test target）: 初期状態が 32 個の hop のモデル（`Init == counter \in 0..31`、`Next == counter' = hop168(counter) % 32`）を、IR で組んで worker 2 で検査する。before の debug は 128 で拒否されるので、Red は after の上限だけを入れて pool の stack を入れない段で、rayon の 2 MiB の worker の stack overflow による abort として記録する。after は complete。
  - `crates/rtla-compiler/tests/call_depth.rs`（新しい）: Rust frontend の TLA+ の hop、N ∈ {41, 76, 84, 85, 168} の complete と N = 169 の拒否、`chain_N50.tla` の complete を、release は 2 MiB、debug は 8 MiB の thread で検査する。before は N ≥ 41 が 128 で拒否されて失敗する。
  - `crates/rtlc/src/safety_definition_tests.rs`: call の 511 / 512 / 513 を 8 MiB の thread で検査する。対照として、再帰する定義の call の連鎖の 127 / 128 / 129 で、127 が受理され、128 と 129 が今と同じ `bounded / evaluation` と work になることを置く（before でも通る対照で、Red ではない）。ENABLED の subscript は、step の数を 3 定義で 512 を超える数にして拒否を、10 で受理を検査する。
  - TS: `tests/call-depth/contract-test.ts` で 4 経路の hop のセルを検査する。`tests/wasm-host/contract.ts` と `tests/portable-browser-entry.ts`、`tests/portable.browser.spec.ts` に、Node と Chromium で共通の `runCallDepth` を足す（N = 168 の complete、N = 169 の型付きの拒否、拒否の後の同じ instance での小さい要求の complete）。
- 期待値が変わる既存のテストを、実装の前にここで宣言する。
  - `safety_definition_tests.rs` の `v2_dynamic_call_depth_is_checked_before_exhausting_the_native_stack`（127 / 128 / 129 を 511 / 512 / 513 にし、8 MiB の thread で動かす）と `v2_enabled_subscripts_keep_the_depth_of_live_branch_steps`（60 では 3 定義で 200 段に満たず受理されるので、step の数を上げる）。
  - `crates/rtlc/tests/evaluation_stack.rs` の境界の大きさと thread の大きさ。
  - `fixtures/nesting-chains/cases-0075.json` の `calls-127`・`calls-128`・`calls-129` の v2（Rust・TS）。`cases-0075.json` と `records-0075.json` は 0075 の記録から導いたもので、変えない。0119 の入力と、この 3 つのセルの before と観測用の複製の出力を `fixtures/call-depth/records-0119.json` に置き、`evals/experiments/call-depth/expectations-0119.ts` が導いた期待値を `fixtures/call-depth/cases-0119.json` に置く。`crates/rtla-compiler/tests/nesting_chains.rs` と `tests/nesting-chains/contract-test.ts` は、`boundary-0075` のこの 3 つのセルだけを `cases-0119.json` の同じ名前のセルで上書きして検査する。
  - 期待値を変えないもの: 再帰の経路のテスト（観測の一覧）、action frame の 48（`deep_request_drop.rs`、unit test）、IR の深さ 128（`deep_drop.rs`、`crates/rtlc/tests/nesting_chains.rs`）、`tests/tlc-differential/analysis-ir-corpus.ts` と `tests/workspace-oracle/contract-test.ts` の 48、`operator_sharing_limits.rs` と `operator-sharing-limits-test.ts` の tuple の 6 と 16 と 20 と 30、`tests/wasm-host/contract.ts` の既存の件数。実装の前に、debug と release の全テストを流して分類を確かめる。分類と違うテストが失敗したら、実装を直すか、評価の前に条件の変更として plan に追記する。
- 合成の入力（公開用に作ったもので、特定の対象の名前を含まない。乱択は使わない）:
  - hop: `evals/experiments/call-depth/sources.ts` が wrapper つきの TLA+ を作る。N ∈ {40, 41, 76, 84, 85, 168, 169}。Quint の生成物は、固定した `chain_N50.tla` だけを生成し直さずに使う（`gen.py` は生成元の記録で、評価では Quint を動かさない）。
  - 種類: 上の 13 種類の 511 / 512 と、stack の測定の大きさ（深さ約 125、250、500）。
  - 意味の対照（深さ 300〜509 の受理される側）: 最も内側の本文の `x \div 0` が、観測用の複製と TLC と同じ最初のエラーになる。評価されない枝の深い呼び出し（`IF counter > 100 THEN hop300(counter) ELSE counter`、`counter > 100 /\ hop300(counter) > 0`）は、深さ 512 を超える式でも拒否されない。`inv == hop168(counter) < 172` の違反の trace が TLC と一致する。
  - 既知の限界の対照: `UNCHANGED Vn` の n ∈ {6, 7, 8, 9}（v1・v2）、`calls-127`・`calls-128`・`calls-129`。
  - 再帰の対照: `Loop(value) == Loop(value)`、`RECURSIVE Sum(_)` で 1..200 を足す定義（before・after とも `bounded / evaluation` で、TLC は complete。残る差として記録する）、hop の深さ約 300 から再帰する定義を呼ぶ入力（before は protocol の 128、after は `bounded / evaluation`）。
- 結果の期待値: status・状態数・生成数・反例は固定 TLC から取る。TLC は 0074・0075 と同じ v1.7.4（revision 5a47802b、jar の sha256 936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88）を `-Xmx256m -Xss8m -workers 1` で 1 つずつ実行し、scratch と metadir をケースごとに分ける。0074・0075 の入力は、入力の sha256 が一致するものだけ [0074-tlc](../runs/0074-tlc/) と [0075-tlc](../runs/0075-tlc/) の結果を引用し、新しい入力だけを実行する。work とエラーの文言は、観測用の複製で同じ IR を検査した値と一致することを要求する。拒否の文言は上の規則から取る。
- 許す差: before で再帰しない経路の `Safety evaluation nesting exceeds 128` になっていた入力が、after で受理されるか、`Safety evaluation nesting exceeds 512` になるか、深さ 128 以上で再帰する定義に入って `bounded / evaluation` になる場合だけ。該当する入力を一覧にして保存する。それ以外の差（v1、`bounded / evaluation` の結果と work を含む）は 0 件を要求する。
- 経路: 4 経路と v1 / v2 で、before と after を流す。0074 の 244 入力と 0075 の入力に 0119 の入力を加える。同じ frontend の native と Wasm の結果の一致を要求する。frontend を変えないので、両 frontend の IR は before と after で byte 単位で同じになることも要求する。
- 既存の入力の結果: 完全応答の `evaluator-corpus`（Quint6・TCommit・共有 chain、8 経路・各 4 iteration）と、旧版 156 モデル・1,560 要求の `just temporal-parity` を before / after で流し、program・結果・work の完全一致を要求する。
- stack: 0075 の harness と同じ形の `evals/experiments/call-depth/stack-harness/` を置く。`kind`（`stack_programs` の 13 種類）と `hop`（TLA+）のモードを持ち、0075-stack-stage と同じく、セルごとに終了コード・signal・stdout・stderr を保存して、parse・compile・check のどこで止まったかを区別する。driver（`evals/experiments/call-depth/stack.ts`）は 1 GiB の出力を基準に 4 KiB の分解能で二分探索し、0075 の `evals/experiments/nesting-chains/stack-harness/cell.ts` を使う。before・after・観測用の複製の debug と release で流す。after の条件は予測のとおり（深さ 511 で release 1,024 KiB 以下、debug 6,144 KiB 以下）。
- Wasm: `evals/experiments/call-depth/wasm-ceiling.ts` が、観測用の複製と after の core を、要求ごとに新しい instance で動かす。Node v24 は既定・`--liftoff-only`・`--no-liftoff` を別の process で流す。Chromium は Playwright で main thread と dedicated Worker の両方を流す。CLI（wasm32-wasip1、`node:wasi`）は hop の 168 / 169 と、観測用の複製での hop と operator の上端を測る。Go adapter（wazero v1.12.0）は hop の 168 / 169 を 1 回流す。どの host でも、観測用の複製の上端が深さ 1,024 以上で、after が深さ 511 を受理し 512 を型付きで拒否することを要求する。
- Criterion: 0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で、0075 と同じ driver・14 関数の kernel（`just evaluator-kernel`）を流す。frontend は変えないので、rtla-compiler の bench は流さない。10% 以上の後退は同条件の ABBA で再確認し、再現する後退を残したまま採用しない。`size_of::<EvalContext>()` を before と after で記録する。
- build・test・benchmark は同時に流さない。

Quint で書かれた実モデルは公開しない。手元で after を流し、128 で止まっていた検査コマンドのうち、通るものと 512 で止まるものの件数と、観測用の複製で測った深さの最大値だけを iteration に記録する（名前・パス・内容は記録しない）。この記録は採否の条件に含めない。

## 記録と完了条件

run は追記のみ。`evals/runs/0119-*` に、source / binary / 入力・driver・実行順・予算・host・終了コードを保存し、失敗も残す。README の規則に従い、commit するのは iteration が引用する summary（表、集計、manifest、終了コード、sha256）だけにし、Criterion の sample、ケースごとの出力、log、大きな dump は消した数・バイト数・digest を `reduced.json` に残す。記録する時点で、実行したホストの絶対パスとユーザー名を placeholder に置き換える。

予定する run は `0119-tlc`、`0119-oracle`、`0119-paths`、`0119-corpus` / `0119-corpus-summary`、`0119-temporal-parity`、`0119-stack`、`0119-wasm-ceiling`、`0119-kernel` / `0119-kernel-summary`、必要なら `0119-abba-N`、`0119-browser`、`0119-evaluate`、`0119-loop`、`0119-audit`。

終了前に次を実行する。

- `just check`、`just wasm-test`、`just safety-test`、`just rtlc-test`、`just compiler-test`、`just quint-test`、`just portable-build` / `just portable-test`、`just go-test`。
- stack のテストを release でも流す: `cargo test --release --locked --manifest-path crates/rtlc/Cargo.toml --test evaluation_stack --test parallel_stack` と `cargo test --release --locked --manifest-path crates/rtla-compiler/Cargo.toml --test call_depth`。
- `just browser` を 1 回流して記録する（Chromium の `runCallDepth` を含む）。
- 共有のテスト（`tests/wasm-host/contract.ts` と browser の entry）を変えるので `just mutation`。
- Rust の実験（stack の harness）を足すので `just runtime-build` / `just runtime-test` と、harness の cargo fmt / clippy。
- `just evaluate` / `just replay-evaluation` と、`just eval-loop` / `just replay-loop`。
- 文書を更新して `just support-check` を通す: `crates/rtlc/SUPPORT.md`（85 の上限の項目、87 の #83 の項目を debug 8 MiB / release 2 MiB の thread の要件に置き換える）、`crates/rtla-compiler/README.md`（15、17）、`crates/rtla-compiler/SUPPORT.md`（66 を結果に合わせて閉じるか書き直す、80）、`docs/rust-frontend.md`（60）、`docs/safety-ir-v2.md`（67 と、評価と検証の段落に上限 512 と再帰の 128 を書く）。

次がそろったときに採用する。

- Red から Green。`parallel_stack` は pool の stack を入れる前の段で debug の abort、入れた後で complete。
- 再現の hop、`chain_N50.tla`、意味の対照の status・状態数・生成数・反例が固定 TLC と一致し、work とエラーが観測用の複製と一致する。N = 169 は全経路で型付きの拒否。
- 許す差の外で、before と after の結果・work・IR に差がない。v1 と再帰の経路は差 0。
- 上の stack と Wasm の条件を満たす。
- 再現する 10% 以上の後退がない。

結果を見る前に、上限の値の代わりを次のとおり決めておく。ある host の Wasm の上端が深さ 1,024 未満、または debug の深さ 511 が 6,144 KiB を超えた場合は、512 を採らない。その場合、同じ条件を 256 で満たす（Wasm の上端が 512 以上、深さ 255 で release 1,024 KiB 以下、debug 6,144 KiB 以下）ときだけ 256 を採り、232 との差が 10% しかないことを iteration に記録する。256 でも満たさなければ、測った値を記録して保留する。値を変えるときは、評価を流す前に条件の変更として plan に追記する。

この変更の後も残る制約: 再帰の経路は深さ 128 で `bounded / evaluation` になり、TLC が受理する深い再帰は未確定のまま。debug の build を 2 MiB の thread で動かす利用者は、約 240 段を超えると abort しうる（要件は 8 MiB）。深さ 512 を超える入力は型付きで拒否し、次の候補は 2、その次が 1。

## 変更するファイル（予定）と差分の見積もり

| ファイル                                                                                           | 内容                                                                                                                              | 行数の見積もり |
| -------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | -------------: |
| `crates/rtlc/src/safety_expressions.rs`                                                            | 上限の分割（再帰 128 / それ以外 512）、u16、葉の検査、ENABLED の見積もり                                                          |          約 20 |
| `crates/rtlc/src/safety_lazy.rs`                                                                   | `Snapshot` の u16                                                                                                                 |              1 |
| `crates/rtlc/src/safety_parallel.rs`                                                               | pool の `stack_size`                                                                                                              |           約 3 |
| `crates/rtlc/src/safety_definition_tests.rs`                                                       | 511 / 512 / 513、再帰の対照、ENABLED の step の数                                                                                 |          約 50 |
| `crates/rtlc/tests/stack_programs/mod.rs`                                                          | 4 種類の追加、temporal の形                                                                                                       |          約 90 |
| `crates/rtlc/tests/evaluation_stack.rs`                                                            | 13 種類の 512 の境界、profile ごとの thread                                                                                       |          約 60 |
| `crates/rtlc/tests/parallel_stack.rs`（新）                                                        | worker の stack                                                                                                                   |          約 70 |
| `crates/rtla-compiler/tests/call_depth.rs`（新）                                                   | hop と `chain_N50.tla`                                                                                                            |         約 120 |
| `crates/rtla-compiler/tests/nesting_chains.rs`                                                     | 3 つのセルの上書き                                                                                                                |          約 15 |
| `tests/nesting-chains/contract-test.ts`                                                            | 同上                                                                                                                              |          約 15 |
| `tests/call-depth/contract-test.ts`（新）                                                          | 4 経路                                                                                                                            |         約 120 |
| `tests/wasm-host/contract.ts`、`tests/portable-browser-entry.ts`、`tests/portable.browser.spec.ts` | `runCallDepth`                                                                                                                    |          約 60 |
| `justfile`                                                                                         | `safety-test` に新しい contract、stack と Wasm の測定の recipe                                                                    |          約 10 |
| `fixtures/call-depth/`                                                                             | `chain_N50.tla`、`records-0119.json`、`cases-0119.json`                                                                           |         生成物 |
| `evals/experiments/call-depth/`                                                                    | `sources.ts`、`expectations-0119.ts`、`stack-harness/`、`stack.ts`、`wasm-ceiling.ts`、Chromium の測定 page、`quint/`（固定入力） |         約 600 |
| 文書（上の 6 ファイル）                                                                            | 上限と thread の要件                                                                                                              |          約 15 |

product code は約 25 行、テストは約 600 行、実験は約 600 行で、fixture と run は生成物。iteration と README の索引は評価の後に書く。
