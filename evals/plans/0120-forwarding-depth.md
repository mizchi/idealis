# 0120: 引数を受け渡すだけの転送を loop でたどり、評価の再帰の段に積まない

[#127](https://github.com/mizchi/idealis/issues/127)を扱う。基準は b57c0a376（origin/main、[#125](https://github.com/mizchi/idealis/pull/125) の merge）。[0119](../iterations/0119-call-depth-limit.md)は rtlc の v2 で再帰しない経路の動的評価の上限を 256 にした。hop の鎖は 1 段あたり 3 段の深さを使うので、N = 84（深さ 257）で拒否される。3 段のうち 1 段は、受け取った引数をそのまま次の operator に渡す `Parameter` の参照の評価に使われる。この iteration では rtlc の v2 の評価だけを変え、引数が `Parameter` / `Capture` だけの転送を、段ごとの再帰ではなく 1 つの loop でたどる（[0119 の plan](0119-call-depth-limit.md) の候補 2）。転送の hop は、評価の再帰の段（`evaluation_depth`）ではなく別の数（転送の段）に積む。意味の変更は 1 点だけである。再帰しない経路の上限 256 は評価の再帰の段だけと比べ、深さから決まるほかの上限と見積もり（再帰する定義に入った経路の 128、葉の検査、ENABLED の subscript の見積もり、遅延関数の snapshot）は、2 つの段の合計（今の深さと同じ値）と比べる。hop ごとの `arena.consume(1)` と深さの判定は今と同じ位置と順序に残し、work の数え方は変えない。結果を変えてよいのは、before の実行のどこかで 256 の拒否が起きた要求だけで、ほかの要求は byte 単位で同じにする。上限の値（256、128）、IR・wire・両 frontend、v1 の数え方と結果、要求の `bounds`、`contracts.ts`、ほかの上限（frontend 40、checker の式 48、IR の深さ 128、frontend の出力検査 128、JSON と TS の decoder 128、parser 64、action frame 48）、rayon の pool の stack（8 MiB）、Wasm の build の設定は変えない。push と remote merge は含めない。

## 改訂

r5: r4 の範囲を限った再確認で残った 2 件を直した。ほかは変えていない。

1. **予算が尽きる並列の検査の集合を、標本の並列の実行だけにした（再確認の指摘 1）。** r4 は集合に、同じ予算の逐次の `check` の結果と一般の `bounded / work` も入れていた。しかし逐次と並列では、候補を受け入れる前に使う work が違う。そのため、並列では常に `bounded / work` になる入力でも、逐次の結果（違反）が集合に入り、並列の誤った違反を合格させてしまう。r5 の集合は、同じ worker の数・同じ予算で before の `check_parallel` を 20 回流して観測した結果だけとする。
2. **並列の検査で hit がある要求では、`published256` と work を比べない（再確認の指摘 5）。** 複製は各 worker の event の列を最初の hit で切るが、product が全範囲で足し合わせる、各 worker の最初の hit までの work は復元しない。上限 4,096 の複製は hit の先も work を使うので、例 C のように判定と trace が合っても work は合わない。r5 は、並列の hit がある要求を判定・エラーの種類と文言・trace で比べ、work は比べない。これはこの比較の限界として残す。逐次の検査で hit がある要求は、今までどおり raw の応答をすべて比べる。

r4: 前提レビュー（r3）の差し戻しを受けた改訂。本文は r4 の内容に置き換えた。r3 から変えた点と理由を次に、それより前の改訂をその下に残す。

1. **並列の検査で予算が尽きる要求を、byte 単位の一致から外した（r3 の指摘 1、Model）。** main でも、並列の検査で共有の予算が尽きる要求の結果は、worker が予算を取る順に依る。例えば `x ∈ 0..15`・worker 2・並列の展開の始めの残り work 1 で、Next の右辺が `IF x = 0 THEN x \div 0 ELSE F2(x)` のとき、結果は最後の 1 を取った worker で決まる。先頭の範囲の worker が取れば `Integer division by zero`、後続の範囲の worker が取れば `bounded / work` になる。これは 0120 が作る差ではなく、今の並列の検査の性質である。r4 は次のように分けた。
   - byte 単位の一致を求めるもの: 逐次の検査と、予算が尽きない並列の検査（その before は、実行の順によらず同じ応答を返す）。
   - 予算が尽きる並列の検査: 結果の種類（status と reason）とエラーの種類が、before が返しうる集合に入ることだけを求める。
2. **`descend` の判定の順を、今のとおりに明記した（指摘 2、Model）。** 順は次のとおりで、再帰の 128 を 256 より先に判定する順も今のままにする（指摘 3 の入力はこの順に依る）。
   1. v1 の早い return。
   2. 再帰する context の 128（合計と比べる）。
   3. 256（評価の再帰の段と比べる）。
   4. 増分。

   r3 の箇条書きは v1 の早い return を最後に書いていた。v1 の program では、早い return より前に 256 を判定すると結果が変わる。自己代入 1 個と真の guard 254 個の v1 の ENABLED では、subscript に渡す深さが 255 + 1 = 256 になる。今は早い return を通って `false` を返すが、順を入れ替えると protocol の拒否になる。この入力を対照にし、順を入れ替える故障を F7 とした。

3. **hit を、実際に起きる 256 の拒否で数え、期待値の分岐にも after の hit を使う（指摘 3）。** 254 段の鎖の終端の `R()` では、after は評価の再帰の段 256・合計 509 で `R()` の本文の入口に来る。そこでは再帰の 128 が先に働くので、正しい結果は work 509 の `bounded / evaluation` である。probe は判定の前に記録するので `peakDepth` は 256 になるが、256 の拒否は起きない。r3 の fixture の規則は `depth`（`peakDepth`）が 256 以上なら拒否を期待したので、ここで誤っていた。r4 は次のように直した。
   - `hits256` は、product なら実際に 256 の拒否を返す判定だけを数える。v1 の早い return を通る判定と、再帰の 128 が先に働く判定は数えない。
   - 期待値の分岐は、`peakDepth` ではなく after の hit の有無で行う。
   - 例 A を 254 段にした（128 段の入力も残す）。
4. **F5c の拒否の位置を直した（指摘 4）。** `select` は、束縛の確認と本文への移行でそれぞれ 1 回 `descend` する。そのため `R_0` の入口は合計 124 になり、最初に 128 になるのは `R_3` の本文で `R_4` を評価する位置である。そこまでの call の課金も直した。
5. **並列の要求の期待値を、集約器の公開の順から決める（指摘 5）。** 集約器は範囲の順に各 worker の event をたどり、候補の違反を先に確定して、後のエラーを公開する前に返る。そのため、全 worker の最初の hit の文言がそろっても、公開の結果は違反になりうる。r4 は次のようにした。
   - 観測用の複製が、自分の実行から「256 の上限があれば公開される応答」（`published256`）を計算し、それを期待値にする。
   - `0120-oracle` の `published256` が before の応答と一致することを、自己検査として求める。
   - 指摘の入力を例 C としてテストにした。
6. **fold の診断を決める規則を、`replacement` が `Some` の最も内側の fold に直した（指摘 6）。** sequence の fold の `replacement` は `None` なので、その中の拒否は書き換えられずに外へ伝わり、外側の集合の fold で書き換えられる。probe の stack は `None` の fold を積まない。集合の union の fold の中に sequence の fold を入れた対照を足した。
7. **エラーの文言の比較先を、セルの種類ごとに決めた（指摘 7）。**
   - after が 256 で拒否し、上限 4,096 の複製が complete になるセル（固定した `hop-168` など）: 複製には比べる文言が無いので、`0120-oracle-after` の `published256` と比べる。
   - 両方が同じ意味のエラーに達するセル: 複製の文言と比べる。
8. **Go の確認の範囲と判定を直した（指摘 8）。** `evals/experiments/call-depth/go/main.go` は `adapters/go` を通らず、wazero で直接 Wasm を instantiate する。trap しても `"trap: …"` を表示して終了コード 0 を返す。r4 では次のようにした。
   - この driver が確かめるのは、wazero の直接の呼び出しまでであると書いた。
   - 採否は、各入力の返答の行を期待値と照合して決める（終了コードでは決めない）。

r2 から r3 で変えた点（記録）:

1. **差を許す要求を、before の実行が 256 の拒否に当たったかで決めた（r2 の指摘 1・2、Model）。** r2 は「256 の拒否以外は全入力で同じ」と書いた。しかし before が 256 で止まった位置から先を after は続けるので、その要求の判定と work は変わりうる。
   - 例 A: `forward` の 128 段の鎖の最も外側の実引数を、本文が `TRUE` の再帰する `R()` にする。main は work 256 で、`R()` の入口で protocol の拒否を返す。after は同じ位置で評価の再帰の段が 129・転送の段が 127 なので `R()` に入り、本文の入口で合計 257 により、work 257 の `bounded / evaluation` になる。
   - 例 B: 並列の検査で、先頭の worker が停止しない再帰の `bounded / evaluation` で止まる要求でも、後続の worker の work が変わる。後続の worker は、main では 256 で止まり、after では転送を終えて次の親状態に進む。並列の評価器は全 worker の work を足してから先頭のエラーを扱うので、公開する reason は同じでも work が変わる。

   r3 の枠組みは次のとおり。
   - 意味: 256 は評価の再帰の段だけと比べ、ほかの深さから決まる上限はすべて合計と比べる。変更はこれだけである。
   - 差を許す要求: before の実行のどこかで 256 の拒否が起きた要求だけ（どの位置でも、どの worker でもよく、fold が書き換えた拒否も含む）。その期待値は、2 つの段を持ち 256 を 4,096 にした観測用の複製（after）と TLC 1.7.4 で決める。
   - ほかの要求: before と byte 単位で同じ。256 の拒否が起きたかどうかは、観測用の複製が要求ごとに数える。

   例 A と B は、期待値つきのテストにした。

2. **`and-forward` の片側の対照にも外側の `And` を残した（指摘 3）。** 単体の `Inv == c198` は、結合した入力より `And` の 1 段だけ浅い（201 と 202）。片側の入力を `fwd100(counter) /\ counter >= 0` と `counter >= 0 /\ c198` にした。
3. **F5 を subscript・step・snapshot に分け、subscript に届く対照にした（指摘 4）。** r2 の 64 段の鎖では、再帰する定義の本文に入った時点で合計が 129 になる。ENABLED は入口で拒否されるので、subscript を壊す故障を観測できなかった。r3 は F5a（subscript）・F5b（step）・F5c（snapshot）を独立に作る。ENABLED の対照は 60 段の鎖にして、`R` と ENABLED に入った後の合計を 122 にし、5 step で literal の subscript の入口だけが 128 になるようにした。
4. **F2 を、まとめた課金を見分けられる対照に直した（指摘 5）。** `Arena::consume` は不足すると残りの予算を使い切る。そのため 6 回の `consume(1)` を `consume(6)` にまとめても、全予算の走査では値・エラー・最終の work が一致する。r3 は F2 を 2 つに分けた。
   - F2a: 鎖ごとに 1 回だけ課金する。全予算の走査で区別する。
   - F2b: 先に鎖の参照をたどり、最後にまとめて課金する。6 call の後の 3 番目の参照を不正にして予算を 7 にした対照で区別する（正しい実装は 2 回目の引数の要求で `budget`、F2b は `Invalid lexical reference`）。
5. **fold の書き換えの曖昧さを、要求ごとの数で決めた（指摘 6）。** 3 状態の入力では、main は状態 0 の深さの拒否を文字列の fold の診断に書き換える。上限 4,096 の版は、状態 2 の 0 除算を同じ診断に書き換える。エラーの応答には止まった状態も work も入らないので、raw の応答は同じになる。r2 の「before と複製の出力の差」では、256 が結果を決めたことを見分けられない。r3 は次のように決める。
   - 差を許すかどうか: 観測用の複製（before）の、要求ごとの 256 の hit の数で決める。
   - after の期待される文言: 観測用の複製（after）が最初の hit で記録した fold の診断（fold の外なら生の拒否）。
     この 3 状態の入力をテストにした（after は状態 1 で止まり、レコードの fold の診断を返す）。
6. **probe を要求ごとに初期化し、判定の前に記録するようにした（指摘 7）。** 0119 の `probe_peak()` は、process 全体に 1 つの、初期化しない最大値だった。4 経路の driver は native の process と Wasm の instance を使い回すので、深さ 300 の次に 40 を流すと 300 が残る。また 0119 の patch は判定の後に記録するので、再帰の葉で初めて 128 に達して拒否されると、記録は 127 になる。r3 の probe は次のとおり。
   - 要求の始めに初期化し、応答に付けて返す。
   - 判定の前に、2 つの段の値・256 の hit の数・最初の hit の出力を記録する。
7. **TLC の照合に要る記録を取り直す（指摘 8）。** 引用する `violation-76` の TLC の記録には `states`・`generated`・`trace` の数しか無く、各状態の `counter` が無い。そこで、trace の状態を保存していない引用元のケースは、固定 TLC で取り直す。エラーの入力（`division-by-zero-120`）は、rtlc のエラーの応答が `code` と `message` だけなので、エラーの種類だけを比べる。
8. **Wasm の再試行の予算を、driver の引数で渡す（指摘 9）。** Node と Chromium の driver は、要求を作るときに `kindBounds.maxWork = 1,000,000` を固定している。受理の判定の引数に加えて、`maxWork` の引数を両方に足す（既定は今と同じ）。
9. **0120 の driver と解析を `evals/experiments/call-depth/` に置く（指摘 10）。** AGENTS.md の規則で、計測と再生のコードは experiments に置く。run には出力と summary だけを残す。

r1 から r2 で変えた点（記録。r2 の 1 の「結果・work とも全入力で before と同じ」は、r3 の 1 で「before の実行が 256 の拒否に当たらない要求」に狭めた）:

1. **深さを 2 つの数に分けた（指摘 1、Model）。** r1 は 1 つの深さから途中の hop の分を除いた。しかしその深さは 256 の判定だけでなく、再帰する定義に入った後の 128 の判定、ENABLED の subscript の見積もり、遅延関数の snapshot にも使われる。そのため、再帰しない転送を終えてから再帰する定義に入る入力で、128 の判定が変わっていた。例えば `forward` の 64 段の鎖の最も外側の実引数を、本文が `TRUE` の再帰する `R()` にすると、今は深さ 129 で `R` の本文に入って `bounded / evaluation` になるが、r1 は 66 で入って complete を返す。r2 は転送の hop を別の数 `forwarded` に積む。128 の判定・ENABLED・snapshot は合計（どの位置でも before の深さと同じ値）で行い、256 の判定だけを評価の再帰の段で行う。#125 が変えないとした再帰の経路は、結果・work とも全入力で before と同じになる。再帰の結果が動くことを許す案（TLC を判定の基準にする）は候補 3 として残し、採らない。
2. **途中の hop ごとに、次の参照と `consume(1)` の前に深さの判定を置いた（指摘 2、Model）。** r1 は再帰しない context で途中の判定を消していた。そのため深さ 256 では、次の slot の参照と `consume(1)` が先に走った。work 切れ（`bounded / work`）や `Invalid lexical reference` が深さの拒否より先に出て、拒否までの work も 256 から 510 に変わった。r2 の判定は今の途中の hop と同じ位置と順序で、比べる値だけが違う。before が拒否した位置で、256 と比べる値も 256 以上なら、同じ位置・同じ work で同じ拒否になる。
3. **深さの予測を位置ごとの値に直した（指摘 3、Model）。** 256 と比べる値は、各位置で「before の深さ − その位置までの経路の途中の hop の数」になる。要求全体の最大は、各位置の値の最大を取ったものである。before の最大の経路から hop の数を引いた値ではない。例えば `forward(100) /\ call(198)` は 201 から 199 になり、201 − 99 = 102 にはならない。この形の unit test を足した。
4. **sweep の照合を probe の定義に合わせた（指摘 4）。** 深さ 128 未満の葉は判定を呼ばないので、葉で終わる入力の記録は構造上の深さより 1 小さい（hop-40 は 125 に対して 124、`forward` の 63 は 126 に対して 125）。r2 は次の 2 つに分けた。
   - 全点の一致: 2 つの観測用の複製の記録どうし（合計の段）で求める。
   - 式との一致: 構造上の深さが 128 以上の点と、受理・拒否の境界に限る。
     また、`violation-120` の after の深さを、hop-invariant と同じ形の 6 + 2N = 246 に直した（r1 は 245）。0119 の `violation-76` の宣言 233 も同じ形なので、正しくは 6 + 3N = 234 になる。
5. **primed の対照を、終端に `Prime` が残る形に直した（指摘 5）。** 両 frontend は `counter'` を `Read { next: true }` にするので、r1 の `F(counter')` は終端の `Prime` を試していなかった。r2 は IR の unit test で `Prime` を手で置き、TLA+ では `fwdN((counter + 0)')` を使う。compile した IR の引数の根が `prime` であることは、fixture の生成で確かめる。
6. **期待値の導き方と許す差を、fold の診断に合わせて直した（指摘 6）。** after の 256 と比べる値が 256 以上のセルは、生の `Safety evaluation nesting exceeds 256` ではなく before の出力を期待する（fold の本文の中で拒否されるなら fold の診断）。文字列の fold の本文で恒等の転送を呼ぶ unit test を足した。
7. **4 経路の比較に、正規化しない応答の照合を足した（指摘 7）。** 0119 の driver はエラーの `code` を捨て、文言の接頭辞と位置を除き、違反の trace を長さに縮める。これではエラーと反例の同一性を検査できない。r2 は raw の応答の sha256 と trace の全状態を比べ、TLC とも trace を状態ごとに比べる。また hop-126 の動的評価の拒否は Rust の 2 経路でしか観測できない（TS は `%` を compile で拒否する）ので、4 経路の境界の拒否は hop-invariant-125 で検査する。
8. **stack と Wasm の採否の点を、生成できる点に直した（指摘 8）。** hop は N = 125（深さ 255）を測る。どの種類でも深さ 255 / 256 を作れるわけではない（`function` は大きさ 62 / 63 で 253 / 257）。そのため、after で受理される最大の大きさと次の大きさを使う。上端の探索は `bounded` を受理に数えるので、採否には `complete` だけを使う。
9. **予算と時間の条件を、再生する既存の入力に合わせた（指摘 9）。** 4 経路に含まれる `LongIfChain-safety` は 56 状態、`evaluator-corpus` の TCommit-8 は 6,816 状態に達する。`temporal-parity` には、`maxWork = 1` の `bounded / work` を返すことが正しい対照もある。r2 は次のように直した。
   - 予算: 各 experiment が記録した値のまま使い、既存の入力では記録した出力（`bounded` を含む）との一致を条件にする。
   - 「32 状態以下」と「`bounded` は不合格」: 0120 の新しい入力にだけ適用する。
   - wall clock の上限: 値で決めた。
10. **`just safety-eval` / `just safety-replay` / `just safety-tlc` を終了前の検査に足した（指摘 10）。** AGENTS.md が safety backend に要求する検査で、`just evaluate` は別の runner である。

## 観測

この plan のための新しい測定はしていない。値は b57c0a376 で固定されたテストと 0119 の記録（[iteration](../iterations/0119-call-depth-limit.md)、[oracle](../runs/0119-oracle/)、[paths](../runs/0119-paths/)、[stack](../runs/0119-stack/)、[wasm-ceiling](../runs/0119-wasm-ceiling/)、[cli-r2](../runs/0119-cli-r2/)）から取った（rustc 1.100.0、aarch64-apple-darwin、Node v24.22.3）。

深さは 0119 の追記と同じく、`descend` が上限と比べる値（下降を試みる直前の深さ）の最大で、深さ 128 以上の葉の検査も含める。上限 L は、この値が L 以上の入力を拒否する。深さ 128 未満の葉は判定を呼ばないので、葉で終わる入力の記録は、構造上の深さが 128 未満のとき構造上の値より 1 小さくなる（0119 の追記、hop-40 は構造上 125、記録は 124）。下の表の深さは構造上の値で、境界（128 以上）では記録と一致する。

| 入力                                                                                | 深さ   | b57c0a376 の境界                                                    | 固定しているもの                                                                                    |
| ----------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| hop（wrapper つき、`step == counter' = hopN(counter) % 5`）                         | 5 + 3N | N = 83（254）が complete 5 / 6・work 862、N = 84（257）が最初の拒否 | `crates/rtla-compiler/tests/call_depth.rs`、`fixtures/call-depth/cases-0119.json`                   |
| hop-invariant（`inv == hopN(counter) >= 0`）                                        | 6 + 3N | N = 83（255）が complete、N = 84（258）が最初の拒否                 | `cases-0119.json`、`tests/call-depth/contract-test.ts`                                              |
| `chain_N50.tla`（固定した Quint 0.33.1 の生成物）                                   | 155    | complete 5 / 6・work 532                                            | `call_depth.rs`                                                                                     |
| `stack_programs` の `forward`（1 定義 1 段の call、引数は呼び出し側の `Parameter`） | 2s     | s = 127（254）が complete・work 512、s = 128（256）が拒否           | `crates/rtlc/tests/evaluation_stack.rs`                                                             |
| 遅延関数を 70 段の転送の先で適用し、本文に 130 段の call を置く                     | –      | 70 と 130 の組み合わせが拒否、片方だけは complete                   | `safety_definition_tests.rs` の `v2_applied_lazy_function_body_counts_the_depth_of_its_application` |
| Rust frontend の Wasm と Chromium の `runCallDepth`                                 | 5 + 3N | N = 83 が complete、N = 84 が型付きの拒否                           | `tests/wasm-host/contract.ts`、`tests/portable-browser-entry.ts`、`tests/portable.browser.spec.ts`  |

- hop の work は、5 状態になる N で 10N + 32、N + 1 が 5 の倍数で 1 状態 / 2 生成になる N で 2N + 8 になる（0119 の plan の表の N = 40、41、76、85、168 と N = 84、169、`call_depth.rs` の N = 83 に一致）。hop-invariant は N によらず 5 状態で、work は 10N + 32（[0119-paths](../runs/0119-paths/) の 84、85、168、169 に一致）。
- `forward` の work は 4s + 4（s = 127 で 512）。`call`（引数のない call の s 段）は 255 が受理・256 が拒否なので、invariant の根の深さは 0 である。`forward` の深さは 2s になる（call が s 段、使う側の `Parameter` が 1 段、途中の転送が s − 1 段で、終端の葉が深さ 2s で比べられる）。
- 0119 の `violation-76`（`inv == hop76(counter) < 80`）は hop-invariant と同じ形だが、深さを hop の式 5 + 3N = 233 で宣言している。同じ形の hop-invariant-76 の宣言は 234 で、0119 の期待値の導き方では 256 未満の側で同じ結果になるので、出力には影響しない。
- 手元のモデル: 0119 の plan の記録では、観測用の複製で測った評価の深さの最大は 264 と 151 だった。264 の側は今も 256 で拒否される。#127 の記録では、このモデルはレコードの状態を約 10 層の operator に引数として受け渡し、各層は受け取った引数をそのまま次へ渡す。

### 転送の再帰の場所

行番号は b57c0a376 のもの。

- `crates/rtlc/src/safety_expressions.rs:874-942` の `eval_reference` のうち、`Parameter` / `Capture` の腕（`:926-939`）が次の順に処理する。
  - `ctx.frame` を取る（`:927-929`、無ければ `Missing lexical call frame`）。
  - `Parameter` なら `frame.arguments`、`Capture` なら `frame.captures` を選ぶ（`:930-934`）。
  - `slot` の `Argument` を取り（無ければ `Invalid lexical reference`、`:935-937`）、その `evaluate` を呼ぶ（`:938`）。
- `Argument::evaluate`（`:60-76`）は `arena.consume(1)`（`:66`）の後、引数の式を渡された evaluator に評価させる。context は `locals`・`frame`・`old` だけを `Argument` のものにし、残りは `..context`（`:69-74`）。引数の式が `Parameter` / `Capture` なら、evaluator は `descend` してから同じ腕に戻る。これが段ごとの再帰で、転送の 1 hop ごとに evaluator・`eval_reference`・`Argument::evaluate` の native の frame が積まれ、深さが 1 段増える。
- evaluator は 3 種類あり、どれも `Parameter` / `Capture` で 1 回 `descend` してから `eval_reference` に入る。
  - `Expression::eval`（`:814-835`、深さ 128 以上の葉の前の検査は `:819-822`）→ `eval_nonleaf`（`:838-871`、`descend` は `:843`、振り分けは `:865-868`）。`eval_operator` の `:1259-1265` も同じ腕を持つが、`eval_nonleaf` が先に振り分けるので通らない。
  - `MembershipSet::eval`（`:2055-2070`、`descend` は `:2061`、`:2062-2069`）。
  - `eval_function`（`crates/rtlc/src/safety_lazy.rs:59`、腕は `:94-100`、`descend` は `:98`）。
- 上限は `descend`（`:117-134`、定数は `:17-18`）。深さ 128 以上で、再帰する context なら `Recursive safety evaluation nesting exceeds 128`（`evaluation-budget`）、そうでなく 256 以上なら `Safety evaluation nesting exceeds 256`（protocol）を返す。v1 の早い return（`:118-120`）は、metadata・定義・frame のどれもない context だけに効く。v1 の検証は `Parameter` / `Capture` を `Operator references require safety IR v2` で拒否する（`:649-662`）。
- `Argument`（`:38-44`）は `expression`・`locals`・`frame`（作った時の呼び出し側の frame）・`old` だけを持つ。`CallFrame::new`（`:79-100`）は、引数か capture がある call で、呼び出し側の frame を複製して親にする（`:84-88`）。`prime`・`current`・`next`・`recursive`・`evaluation_depth` は `Argument` に入らず、使う側の context から来る。
- `recursive` は `Call` が `ctx.recursive || definition.recursive` で立てる（`:917`、action の `Step::Call` は `:2386`）。ENABLED の step の context（`:1205-1214`）と subscript の context（`:1219-1228`）は、呼び出し側の `frame`・`locals`・`recursive` を引き継ぐ（subscript の `recursive` は 61468f8bc で足した `:1226`）。

hop の 1 段（`hopK(x) == hop(K-1)(x) + 1`）の 3 段は次のとおり。

- `Call` の `descend`。
- 本文の `Add` の `descend`。
- 最も内側の `hop0` の本文で `x` を読むときの、転送の鎖の途中の hop の `descend`。`hop0` の `x` は `hop0` の frame の `Argument`（式は `hop1` の `Parameter 0`）を指し、それは `hop1` の frame の `Argument`（式は `hop2` の `Parameter 0`）を指す。N 個の途中の hop を経て、`step` が渡した `Read counter` に着く。

途中の hop が行うことは、深さの判定と 1 段の増分、frame と `Argument` の参照、`consume(1)` だけである。tick・cache・snapshot・値の生成はない。次の hop は `locals`・`frame`・`old` を自分の `Argument` のもので上書きし、ほかの field は使う側のものをそのまま引き継ぐ。

### 1 つの深さを使うもの

今の `evaluation_depth` は 1 つの数で、次の 5 か所が読む。途中の hop の段を除くと、次の全部が変わる。

- 再帰しない経路の 256 の判定（`descend`）。native の再帰の深さを抑える上限で、#127 が変えたいのはこれだけである。
- 再帰する定義に入った経路の 128 の判定（`descend`）。`bounded / evaluation` を返す、停止しない再帰の予算。
- 葉の前の検査（`:819-822`、深さ 128 以上で `descend` に判定を任せる）。
- ENABLED の subscript の深さの見積もり（`:1222-1224`、`ctx.evaluation_depth + steps + 1`）。
- 遅延関数の `Snapshot`（`safety_lazy.rs:7-44`）。作った時の深さから本文を評価する。

### 上限と転送に依存するもの

- **期待値が変わるもの**（転送を含み、深さ 256 の手前にある入力）:
  - `evaluation_stack.rs` の `forwarded_parameters_reach_the_nesting_limit_within_the_required_stack`（127）。
  - `safety_definition_tests.rs` の `v2_applied_lazy_function_body_counts_the_depth_of_its_application`（70 / 130）。
  - `call_depth.rs` の `layered_calls_complete_up_to_the_nesting_limit` と `layered_calls_past_the_nesting_limit_are_refused`（84、85 を拒否と書いている）。
  - `tests/call-depth/contract-test.ts`（`cases-0119.json` の hop と hop-invariant の 84・85 を拒否とし、拒否の後の instance の検査に hop-invariant-84 を使う）。
  - `tests/wasm-host/contract.ts`・`tests/portable-browser-entry.ts`・`tests/portable.browser.spec.ts` の `runCallDepth`（83 / 84）。
- **転送を含まないので変わらないもの**: `evaluation_stack.rs` のほかの 12 種類、`parallel_stack.rs`（membership の入れ子）、`v2_dynamic_call_depth_is_checked_before_exhausting_the_native_stack`、`v2_recursive_call_depth_keeps_the_former_limit`、`v2_enabled_subscripts_keep_the_depth_of_live_branch_steps`、`v2_enabled_subscripts_keep_the_recursive_limit`、`calls-127〜129`、`UNCHANGED Vn`（0 引数の定義）。
- **再帰の経路で 128 を固定しているもの**（0119 の一覧と同じ。r2 ではすべて変わらない）: `crates/rtla-compiler/tests/recursive.rs`、`recursive_actions.rs`、`crates/rtlc/tests/recursive_actions.rs`、`dead_bindings.rs`、`fold_order.rs`、`nesting_chains.rs`、`tests/frontend-semantics/`、`tests/recursive-actions/`、`tests/tla-coverage/`、`tests/tlc-differential/operator-sharing-test.ts`、`tests/wasm-host/contract.ts` と `tests/portable.browser.spec.ts` の `evaluation`、`cases-0119.json` の `recursion-loop`・`recursion-sum-200`・`recursion-from-hop-50`。`recursion-from-hop-50` は再帰する `Sum` の本文で 51 hop の転送を読むので、転送の段の数え方で再帰の結果と work が決まる入力である。
- **work を固定しているもの**: `v2_nested_identity_has_exactly_four_units_of_work`、`v2_repeated_parameter_reads_reexecute_the_argument_without_a_value_cache`、`v2_except_old_requires_explicit_capture_and_keeps_its_lexical_value`、[0119-corpus](../runs/0119-corpus/) の `evaluator-corpus`、[0119-temporal-parity](../runs/0119-temporal-parity/)、`fixtures/` の記録。`crates/rtla-compiler/README.md` は、call entry と遅延引数・capture の要求が work を 1 ずつ使うと書いている。
- **エラーの文言を置き換えるもの**: 集合の fold の本文（`eval_fold`、`:1534`）は、`budget`・`evaluation-budget`・`Unsupported TLA input` 以外のエラーを fold の診断の文言に置き換える（`:1571-1580`）。深さの拒否が fold の本文の中で起きる入力では、出力は `Safety evaluation nesting exceeds 256` ではなく fold の診断になる。
- **文書**: `crates/rtlc/SUPPORT.md`（85、87）、`docs/safety-ir-v2.md`（53）。数（256、128）は変わらないので、`docs/rust-frontend.md`、`crates/rtla-compiler/README.md`、`crates/rtla-compiler/SUPPORT.md` は変えない。

## 候補

簡単な順に並べる。どれも、TLC に揃えた評価順・短絡・最初に報告するエラー・work を変えないことを前提にする。

1. **上限を 512 に上げ直す。** 0119 で、Chromium の dedicated Worker の Wasm の上端が update の入れ子で深さ 806（最終の複製で 827）しかなく、512 に求める 1,024 に届かなかった。採らない。
2. **転送の鎖を loop でたどり、hop を転送の段に積む（採る、0119 の候補 2）。**
   - 途中の hop の再帰をなくし、hop は評価の再帰の段ではなく転送の段に積む。256 は評価の再帰の段と比べる。128・葉の検査・ENABLED・snapshot は合計（今の深さ）と比べる。
   - hop ごとの判定と `consume(1)` は今と同じ位置と順序に残す。
   - 効果: hop の鎖は 256 と比べる値で 1 段あたり 2 段になり、上限 256 のままで N = 125 まで受理する。評価の再帰も hop ごとに減る。
   - 結果が変わるのは、before の実行が 256 の拒否に当たった要求だけ（例 A・B を含む）。
   - 費用: `EvalContext` と `Snapshot` に u16 の field が 1 つ増える。
3. **1 つの深さのまま途中の hop を数えず、深さから決まる結果が動くことを許す（r1 の案）。**
   - コードは最も小さく、動いた結果は TLC の答えか型付きの予算切れになる。
   - しかし before が 256 に当たらない要求でも結果が変わる。再帰しない転送の後に入った再帰の `bounded / evaluation` が、complete か別の work の `bounded` に変わる。例えば `forward` の 64 段の鎖の終端の `R()` は、before の深さが 256 に届かないのに complete になる。
   - 0119 が再帰の経路に要求した「結果と work の差 0」と、#125 の「再帰の 128 は変えない」を保てない。
   - 判定と増分を分けて途中の判定だけを残しても、128 の判定の前の深さは下がったままである。
   - 採らない（最終的な判断は maintainer への問い 2）。
4. **program に再帰する定義が 1 つも無いときだけ、途中の hop を数えない。** 再帰の結果は変わらず、変更は Arena の 1 つの印で済む。ただし使われない `RECURSIVE` の定義を 1 つ足すだけで、深い転送の入力の受理が変わる。結果がその入力の外の宣言に依るので、採らない。
5. **使う側の `Parameter` / `Capture` の 1 段も数えない。** 転送でない参照（引数の葉を読むだけの参照）の深さも変わり、終端の式を評価する深さの意味が「参照の 1 段先」でなくなる。#127 の再現に要るのは途中の hop だけなので、採らない。
6. **`CallFrame::new` で転送を畳む（作る時点で、`Parameter` / `Capture` の引数を親の `Argument` の複製に置き換える）。**
   - 使う時点の hop ごとの `consume(1)` がなくなるか、まとめて数えることになり、work と予算切れの位置が変わる。
   - 使われない引数の参照のエラーが、使う時点ではなく call の時点で起きる。
   - `Step::Call`・`TemporalFormula::Call` の frame も変わり、影響の範囲が広い。
   - 採らない。
7. **frontend で転送だけの operator を別名に畳む（`f(x) == g(x)` を `g` にする）。** IR が変わり、両 frontend の照合、保存した IR、work が変わる。採らない。
8. **引数の値を frame ごとに cache する。** `v2_repeated_parameter_reads_reexecute_the_argument_without_a_value_cache` が固定する「値の cache を持たない」と、work の数え方を変える。採らない。
9. **`Update` と `At` の spine を反復でたどる（0119 の候補 1）。** hop には spine がないので、#127 の再現の深さは変わらない。次の候補として残す。
10. **評価器全体を heap 上の明示的な stack に移す。** 評価器の書き直しになる。対象外。

## 仮説

意味の変更は 1 点だけとする。再帰しない経路の上限 256 は評価の再帰の段（`evaluation_depth`）だけと比べ、深さから決まるほかの上限と見積もりはすべて合計と比べる。rtlc の v2 でこれを次の形で実装すれば、hop の鎖の評価の再帰の段は 5 + 2N になる。

- 転送だけの引数を loop でたどる。
- 途中の hop を転送の段に積む。

上限 256 のままで、N = 125（255）は固定 TLC と同じ 5 状態 / 6 生成で、work 10N + 32 = 1,282 の complete になる。N = 126（257）は `Safety evaluation nesting exceeds 256` の型付きの拒否になる。

after は、before の実行が最初に 256 の拒否に当たる位置までは、before と同じ評価の手順をたどる。同じ式を同じ順序で評価し、同じ `consume` を呼び、同じ位置で判定する。各位置で合計の段は before の深さと等しく、違いは 256 と比べる値だけである。したがって次のとおりになる。

- before の実行のどこでも 256 の拒否が起きない要求は、応答が byte 単位で同じになる（どの worker でもよく、fold が書き換えた拒否も含めて、拒否が起きないこと）。同じになるものには、work、再帰の `bounded / evaluation`、work 切れ、エラー、違反の trace、予算が尽きない並列の検査を含む。予算が尽きる並列の検査は、今も worker が予算を取る順で結果が変わるので、結果の種類とエラーの種類で比べる。
- 256 の拒否が起きた要求では、after はその位置から先を続けることがあり、判定・work・エラーが変わりうる。その期待値は、観測用の複製（after、上限 4,096）と固定 TLC で決める（下の「変えないものと、変わってよいもの」）。

### 転送だけの引数の規則

転送とは、`CallFrame` の `arguments` か `captures` に入った `Argument` のうち、式の根が `Expression::Parameter { slot }` か `Expression::Capture { slot }` であるものをいう。式全体が 1 つの参照であることだけを条件にし、ほかの形は転送に含めない。

`EvalContext` に、転送の段 `forwarded`（u16、飽和加算、名前は仮）を足す。`evaluation_depth` は評価の再帰の段（native の evaluator の再帰に対応する段）を数え、転送の hop は `forwarded` を 1 増やす。合計 `evaluation_depth + forwarded`（飽和加算）は、どの評価の位置でも before の `evaluation_depth` と同じ値になる。

- `descend` は、今と同じ次の順で判定する（今の `:117-134` の順のまま、比べる値だけを替える）。
  1. v1 の早い return: metadata・定義・frame のどれもない context なら、判定も増分もせずに戻る。
  2. 合計が 128 以上で、再帰する context なら `Recursive safety evaluation nesting exceeds 128`（evaluation-budget）。
  3. `evaluation_depth` が 256 以上なら `Safety evaluation nesting exceeds 256`（protocol）。
  4. 判定を通れば `evaluation_depth` を 1 増やす。

  2 を 3 より先に置くのは今と同じで、再帰する context で評価の再帰の段が 256 に達した位置は、`bounded / evaluation` になる（例 A）。1 を最初に置くのも今と同じで、v1 の ENABLED の subscript は見積もりが 256 でも拒否されない。途中の hop の判定も、増分を除いて同じ順で行う。`evaluation_depth` は 256 を超えないので飽和しない。`forwarded` が飽和しても合計の判定は 128 以上のままなので、どちらの判定も見逃さない。

- 葉の前の検査（`:819-822`）は、合計が 128 以上のときに判定を呼ぶ。
- ENABLED の step と subscript の context（`:1205-1214`、`:1219-1228`）と遅延関数の `Snapshot`（`safety_lazy.rs:7-44`）は、`forwarded` も引き継ぐ。subscript の見積もりは、今と同じく `evaluation_depth` に steps + 1 を足す。合計は before の見積もりと同じになる。
- `EvalContext::new` では `forwarded` は 0。

使う側（`Parameter` / `Capture` の式を、上の 3 種類の evaluator のどれかが評価する場所）は、今と同じく 1 回 `descend` してから `eval_reference` に入る。`eval_reference` の `Parameter` / `Capture` の腕は、次の loop で終端の `Argument` を求め、終端の式だけを渡された evaluator で評価する。

1. 使う側の context の `frame` から、式の種類で `arguments` か `captures` を選び、`slot` の `Argument` を取る。frame が無ければ `Missing lexical call frame`、slot が無ければ `Invalid lexical reference`（今と同じ文言と種類）。
2. `arena.consume(1)` を呼ぶ。今の `Argument::evaluate` と同じ位置で、hop ごとに 1 回呼ぶ（まとめて `consume(n)` にしない。並列の検査の共有予算も同じ回数の呼び出しになる）。
3. その `Argument` の式が転送なら、次の hop に進む。
   - まず判定する。次の参照に使う frame（その `Argument` の `frame`）を持つ context で、`descend` と同じ判定（合計 128 と再帰、`evaluation_depth` 256、v1 の早い return）を、増分なしで行う。今の途中の hop が、次の式に入るときに行う判定と同じ位置・同じ順序になる。違いは `evaluation_depth` が増えていないことだけで、再帰する context でも再帰しない context でも規則は同じ。
   - 次に `forwarded` を 1 増やす（`evaluation_depth` は変えない）。
   - その `Argument` の `frame` から、次の hop の式の種類で `arguments` か `captures` を選び、1 と同じ規則で次の `Argument` を取って 2 に戻る。
4. 転送でなければ、その式を終端として評価する。
   - `locals`・`frame`・`old` はその `Argument` のものにする。
   - `current`・`next`・`prime`・`recursive`・`evaluation_depth` は使う側の context のものにする（今の `..context` と同じ）。
   - `forwarded` は使う側の値に、通った途中の hop の数を足したものにする。
   - 終端の式は、自分の判定（葉の検査か `descend`）を通る。

順序は今と同じ「参照 → `consume(1)` → 次の式の判定 → 参照 → …」である。before が深さの拒否を返した位置 p で、after の 256 と比べる値も 256 以上なら、after も p で、同じ work で同じ拒否を返す。after が p を越えて評価を続けるのは、p までの経路に途中の hop があり、256 と比べる値が 256 未満になるときだけである。

loop は `Argument.frame`（作った時の親の frame）をたどる。frame は作った後に変わらない Arc の木なので循環せず、hop ごとに work を 1 使うので `maxWork` でも止まる。loop は別の `#[inline(never)]` の関数にして、すべての call が通る `eval_reference` の frame を大きくしない（0075 と #83 と同じ理由）。`MembershipSet::eval` と `eval_function` は `eval_reference` を呼ぶだけなので、loop のためには変えない（`safety_lazy.rs` は `Snapshot` の field だけを変える）。

規則が覆う形:

- **`Parameter` と `Capture` の入れ子の鎖**: 3 の式の種類は hop ごとに決める。`Parameter` だけの鎖、`Capture` だけの鎖、交互の鎖（LET の定義が外側の引数を capture し、それを次の operator に `Parameter` として渡す形）を、同じ loop でたどる。
- **frame を持つ引数**: どの `Argument` も、作った時の呼び出し側の frame を持つ。loop はそれを、今の `EvalContext { frame: self.frame.as_deref(), .. }` と同じ規則でたどる。
  - `Expression::Frame`・`Step::Frame`・`TemporalFormula::Frame` の無名 frame も同じ `Argument` を作るので、無名 frame を通る鎖もたどる。
  - 親の frame が無い `Argument`（frame の外の call の引数）で式が転送なら、今と同じく判定の後に `Missing lexical call frame` になる。
  - 引数の式が `Frame` や `Call` そのもの（引数の中の LET、`f(g(x))`）なら、終端である。
- **ENABLED**: ENABLED の step と subscript の context は、今と同じく呼び出し側の `frame`・`locals`・`recursive` に加えて `forwarded` を引き継ぐ。subscript の見積もりの合計は before と同じになる。再帰する定義の中の subscript（61468f8bc）も、転送を終えてから再帰する定義に入った後の ENABLED も、128 の判定は before と同じ値で行う。
- **primed な context**: `prime` は `Argument` に入らず、使う側の context から来る（今も `..context`）。
  - `(F(x))'` のように使う側が primed なら、終端の `Read` は次の状態を読む。
  - 根が `Prime` の実引数（`F((x + 0)')` や手で組んだ IR の `Prime { Read }`）は終端で、今と同じく評価する。その中の `Parameter` は新しい使う側になり、そこから loop が始まる。
  - `F(x')` の `x'` は、両 frontend が `Read { next: true }` にするので葉の終端になる。
- **再帰の上限（61468f8bc を含む）**: 128 の判定は、合計（before の深さ）で、今と同じ位置で行う。before の実行が 256 の拒否に当たるまでの範囲では、再帰しない転送の後に再帰する定義に入る入力も、転送の途中で再帰する context にいる入力も、深さ・拒否の位置・その時の work は今と同じになる。before が 256 で止まった位置の先で初めて再帰する定義に入る入力（例 A）は、after で 128 の判定に当たることがある。

転送に含めない形と理由:

- **根が `Prime` の式（`Prime { Parameter }` など）**: `Prime` は残りの評価の `prime` を変える操作で、`descend` を 1 段使う。含めると loop が `prime` を持ち運ぶことになり、規則が「受け渡すだけ」でなくなる。再現と手元のモデルの形（状態を受け渡す層）に、primed な転送はない。
- **`Call`・`Frame` の式**: frame を作り、`consume(1)` と `recursive` の更新を伴う呼び出しで、転送ではない。
- **葉（`Read`・`Bound`・`Old`・`Literal`・`Data`）**: 鎖の終端で、今の葉の評価（合計 128 以上での判定を含む）をそのまま使う。
- **値を作る式（`At`・`Update`・`Record`・算術・比較・`ENABLED` など）**: 値を計算する評価で、深さを数える対象である。spine は 0119 の候補 1 の範囲。
- **`Step::Call`・`TemporalFormula::Call` そのもの**: frame を作る側で、変えない。その中で読む `Parameter` は式の評価なので、loop を通る。

### 変えないものと、変わってよいもの

要求は、観測用の複製（before、`0120-oracle`）が要求ごとに数える 256 の hit で分ける。hit とは、product の before なら実際に `Safety evaluation nesting exceeds 256` を返す判定のことである。v1 の早い return を通る判定と、再帰する context で合計が 128 以上のため先に `evaluation-budget` になる判定は、hit ではない。`0120-oracle` は、256 で止まらないこと以外は before と同じ手順をたどる。そのため hit の数が 0 の要求は、before が一度も 256 の拒否に当たらなかった要求である。1 以上の要求は、before のどこかで 256 の拒否が起きた要求である（どの worker でもよく、fold が書き換えた拒否も含む）。

hit の数が 0 の要求（変えないもの）:

- **応答**: before と byte 単位で同じ raw の応答になる。status・状態数・生成数・違反の trace・work、エラーの code と文言、再帰の `bounded / evaluation`、work 切れを含む。これは逐次の検査と、予算が尽きない並列の検査に当てはまる。
- **予算が尽きる並列の検査**: 今の並列の検査は、共有の予算が尽きると、どの worker が最後の予算を取るかで結果が変わる（改訂 r4 の 1 の例）。これは 0120 の前からある性質で、0120 は変えない。
  - この要求では、after の結果の種類（status と reason）とエラーの種類が、before が返しうる集合に入ることだけを求める。
  - 集合は、同じ worker の数・同じ予算で before の `check_parallel` を 20 回流して観測した結果の種類だけとする。逐次の `check` の結果と、観測していない `bounded / work` は入れない（逐次と並列では候補を受け入れる前に使う work が違うため）。標本による確認で、集合を網羅したとは扱わない。
  - 予算が尽きるかどうかは、`0120-oracle` の work が予算に達したか（`bounded / work` を含む）で決める。
  - 0120 の入力と、再生する既存の入力に、この形の要求はない見込み。出たら一覧に残す。
- **手順**: after は before と同じ式を同じ順序でたどり、同じ位置で `consume(1)` と判定を行う。各位置で合計の段は before の深さに等しい。
- **work**: hop ごとに `consume(1)` を同じ順序で 1 回呼ぶので、予算切れの位置と work も同じになる。並列の検査の共有予算（`WorkBudget::consume`）への呼び出しの回数も同じ。
- **ENABLED と再帰の上限**: 128 の判定、葉の検査、ENABLED の subscript の見積もり、snapshot から再開する深さは、合計で before と同じ値になる。61468f8bc の `recursive` の引き継ぎも、`Recursive safety evaluation nesting exceeds 128` の結果と work も変わらない。

hit の数が 1 以上の要求（変わってよいもの）:

- before が最初に 256 の拒否に当たった位置 p までは、上と同じ手順になる。p で after の 256 と比べる値も 256 以上なら、after も p で同じ拒否を返す（同じ work。fold の本文の中なら同じ fold の診断）。256 未満なら、after は p を越えて評価を続ける。
- 期待値は、観測用の複製（after、`0120-oracle-after`）の `published256` で決める。複製は 2 つの段を持ち、256 を 4,096 にしたもので、`published256` は「256 の上限を持つ product なら公開する応答」を、複製が自分の実行から計算したものである（下の probe の節）。逐次の検査では、after の raw の応答は `published256` と byte 単位で同じでなければならない。並列の検査で hit がある要求は、`published256` と判定・エラーの種類と文言・trace で比べ、work は比べない。複製は各 worker の event の列を最初の hit で切るが、product が全範囲で足し合わせる各 worker の最初の hit までの work は復元しないからで、これはこの比較の限界である。予算が尽きる並列の検査は、上と同じく結果の種類とエラーの種類で比べる。
  - `0120-oracle-after` の hit の数が 0 のとき: `published256` は複製自身の応答である。after は 256 の拒否に当たらず、複製と同じ結果（complete・違反・0 除算などのエラー・`bounded / evaluation`）に達する。判定は、TLC と比べられるものなら固定 TLC とも一致しなければならない。`bounded / evaluation` は型付きの予算切れで、TLC が complete でも、残る差として記録する（例 A）。
  - hit の数が 1 以上のとき: after は 256 の拒否に当たる。`published256` は、逐次の検査なら最初の hit の位置の出力になる。これは、その位置を囲む fold のうち `replacement` が `Some` の最も内側のものの診断で、そのような fold が無ければ `Safety evaluation nesting exceeds 256` である。並列の検査では、集約器の公開の順で決まり、先に確定した違反になることもある（例 C）。複製自身の応答（4,096 まで続けた結果）とは比べない。
- 2 つの観測用の複製は、合計の深さが 4,096 未満の要求で同じ出力になる。128 の判定・葉の検査・ENABLED・snapshot は合計で同じ値になり、違うのは 4,096 の判定（before の深さと比べるか、評価の再帰の段と比べるか）だけだからである。そのため、実装の前の期待値は `0120-oracle` から取り、実装の後に `0120-oracle-after` で確かめる。
- **例 A**（`forward` の 254 段の鎖の終端が、本文が `TRUE` の再帰する `R()`）:
  - before: 254 call と 2 回の引数の要求の後、work 256 で `Safety evaluation nesting exceeds 256`（hit は 1 以上）。
  - after: 254 call と 254 回の引数の要求の後、評価の再帰の段 255・転送の段 253 で `R()` に入る（call の課金で work 509）。本文 `TRUE` の入口は評価の再帰の段 256・合計 509 で、2 の再帰の 128 が 3 の 256 より先に働く。結果は work 509 の `evaluation-budget`（公開の結果は `bounded / evaluation`）。`peakDepth` は 256 だが、`hits256` は 0。
  - 2 つの複製: after と同じ work 509 の `evaluation-budget`。
  - TLC: complete。残る差として記録する。
  - 128 段の同じ形の入力も残す。before は work 256 の protocol の拒否、after と複製は work 257 の `evaluation-budget`（`peakDepth` は 130）。
- **例 B**（並列、worker 2、初期状態 `x ∈ 0..31`。`x = 0` は停止しない再帰、ほかの状態は `forward(200)` を通る自己遷移）:
  - before: `x = 0` を含む worker が `bounded / evaluation` で止まり、もう一方の worker は最初の親状態の 256 で止まる。
  - after: 後者の worker が転送を終えて、残りの親状態にも進む。公開の結果はどちらも先頭の worker の `bounded / evaluation` で、work は after の方が多い。
  - 期待値: after の work は、同じ `check_parallel` を `0120-oracle-after` で流した値と同じ（`0120-oracle-after` の hit は 0）。
- **例 C**（並列、worker 2、初期状態 `x ∈ 0..15`、十分な work、`Inv == x < 16`。Next は `x = 0` なら `x' = forward200(x) + 16`、ほかの状態は `x' = forward300(x)`）:
  - before: 先頭の範囲の最初の親状態 `x = 0` で 256 に当たり、公開の結果は protocol の拒否。
  - after: `x = 0` から候補 16 を作り、その後、両方の範囲で `forward300` の 256 の拒否に当たる。集約器は先頭の範囲の event を順にたどり、候補 16 の invariant の違反を確定して、後のエラーを公開する前に返る。公開の結果は違反（trace は 2 状態）。
  - 期待値: `0120-oracle-after` の `published256`（違反）（判定・エラーの種類と文言・trace で比べ、work は比べない）。全 worker の最初の hit の文言はそろうが、公開の結果はそれではない。TLC の違反の trace とも比べる。

深さ: 256 と比べる値は、各位置で「before の深さ − その位置までの経路の途中の hop の数」になり、before の深さ以下である。等しくなるのは、その位置までに途中の hop を通らない位置だけ。要求全体の最大は位置ごとの値の最大を取ったもので、before の最大の経路から hop の数を引いた値とは限らない。

### 予測（結果を見る前に固定する）

下の深さは、断りがなければ after の 256 と比べる値（評価の再帰の段）。

- **一般の式**: 1 段あたり before で c 段の深さを使い、そのうち f 段が途中の hop である鎖は、after で 1 段あたり c − f 段になる。after の値を b + (c − f)・N と書くと（b は鎖の外の段と、使う側と終端の分を含む定数）、受理される最大の段数は ⌊(255 − b) / (c − f)⌋ になる。
  - hop: b = 5、c = 3、f = 1。
  - hop-invariant: b = 6。
  - `forward`: b = 1、c = 2、f = 1（before は 2s で、途中の hop は s − 1 個）。
  - 鎖の外に別の深い経路がある入力では、その経路の値と比べて大きい方を取る。
- **hop（wrapper つき）**: 深さは 5 + 2N。受理される最大は N = ⌊(255 − 5) / 2⌋ = 125（深さ 255）、最初の拒否は N = 126（深さ 257）。
  - Rust の API と Rust frontend の 2 経路の v2: N ∈ {40, 41, 76, 83, 85, 125} は complete 5 / 6（work 10N + 32: 432、442、792、862、882、1,282）。N ∈ {84, 124} は complete 1 / 2（work 2N + 8: 176、256）。N ∈ {126, 168, 169} は `Safety evaluation nesting exceeds 256`。
  - TS の 2 経路は `%` を持たないので、0119 と同じく compile で拒否する。
- **hop-invariant**: 深さは 6 + 2N。受理される最大は N = 124（深さ 254）、最初の拒否は N = 125（深さ 256）。4 経路の v2 で、N ≤ 124 は complete 5 / 6（work 10N + 32、N = 124 で 1,272）、N ∈ {125, 126, 168, 169} は拒否。
- **`chain_N50.tla`**: 深さは 105 で、complete 5 / 6・work 532 のまま。
- **v1 と CLI**: v1 は 0075 の記録と同じ層（frontend の 40）で、before と同じ。CLI は v1 だけを出すので、0119-cli-r2 と同じになる（hop は 16 まで complete で 17 が `expression or operator nesting exceeds 40`、operator は 17 まで complete で 18 が同じ拒否）。
- **`stack_programs` の転送の種類**:
  - `forward`: 深さは s + 1。受理される最大は s = 254（work 4・254 + 4 = 1,020）。s = 255 は最初の途中の hop の判定で拒否され（work 256、before と同じ位置）、s = 256 は使う側で拒否される（work 256）。
  - 新しい `capture`・`mixed`: `forward` と同じ深さと大きさになる。
  - `forward-set`・`forward-function`: before の深さ 2s + c（c は種類の定数で、観測用の複製（before）の記録から決める）に対して、after は s + 1 + c になる。
  - 転送を含まない 12 種類: 深さ・大きさ・work が今の `evaluation_stack.rs` と同じ。
- **再帰の経路**:
  - 0119 の記録にある `bounded / evaluation`（`recursion-loop` 128、`recursion-sum-200` 702、`recursion-from-hop-50` 130）は hit が 0 なので、結果と work が変わらない。
  - `forward` の 64 段の鎖の終端の `R()` は hit が 0 で、before・after とも `evaluation-budget`・work 129。
  - 例 A（254 段）は before が work 256 の protocol の拒否、after が work 509 の `evaluation-budget`（`peakDepth` 256、`hits256` 0）。128 段は after が work 257 の `evaluation-budget`。
  - v1 の ENABLED（自己代入 1 個と真の guard 254 個、subscript の見積もり 256）は、今と同じく `false` を返す。
- **並列**: 例 B は公開の結果がどちらも `bounded / evaluation` で、after の work は `0120-oracle-after` の値と同じになり、before より多い。例 C は before が protocol の拒否、after が違反（trace 2 状態）。
- **遅延関数の test**: `v2_applied_lazy_function_body_counts_the_depth_of_its_application` の 70 段の転送と 130 段の本文の組み合わせは、途中の 69 hop が 256 と比べる値に積まれなくなり、complete になる。
- **既存の 378 入力**（0074 の 265、0075 の 81、0119 の 32）:
  - hit が 1 以上になるのは、hop・hop-invariant の 84・85・168・169、`UNCHANGED V8` だけの予測。
  - v2 の結果が before と変わるセルは 12 個だけ: hop-84・hop-85 の Rust の native と Wasm（4 セル）と、hop-invariant-84・hop-invariant-85 の 4 経路（8 セル）。変わった先は観測用の複製の出力と同じ。
  - hop-168・hop-169・hop-invariant-168・hop-invariant-169・`UNCHANGED V8` は、after でも hit が 1 以上で、同じ拒否のまま。
- **既存の完全応答**: `evaluator-corpus` と `temporal-parity` は hit が 0 の予測で、program・結果・work が byte 単位で一致する。IR は両 frontend の v1 / v2 で before と byte 単位で同じ。
- **stack**: after で受理される最大の大きさ（深さ 255 以下）での最小の stack は、どの種類も release で 1,024 KiB 以下、debug で 6,144 KiB 以下になり、0119 の最大（release 276 KiB、debug 2,116 KiB）を超えない。
- **Wasm**: 観測用の複製（after）の受理の上端は、どの host・どの種類でも、256 と比べる値で 512 以上。最も浅いのは今と同じ update の入れ子（Chromium の dedicated Worker で約 827）。after は全セルで、受理される最大の大きさを受理し、次の大きさを型付きで拒否する。
- **`size_of::<EvalContext>()`**: 72 byte のまま（u16 の field が padding に収まる見込み）。増えた場合は記録し、stack と Wasm の条件で判断する。
- **Criterion**: 再現する 10% 以上の後退はない。

## 固定条件と対照

### 比べるもの

- **before と after**: before は b57c0a376 の source / binary。after は before にこの変更だけを加えたもの。それぞれの sha256 を記録する。
  - 変わらないことを sha256 で確かめるもの: `crates/rtla/src`、`crates/rtla-compiler/src`、`crates/rtlc/src/ir.rs`、`crates/rtlc/src/safety_parallel.rs`、`packages/idealis/src`、`crates/idealis-core/src`、`adapters/go`。before・after・評価の終了時で比べる。
  - 変わる product code は `crates/rtlc/src/safety_expressions.rs` と `crates/rtlc/src/safety_lazy.rs`（`Snapshot` の field だけ）とする。
- **観測用の複製**: 2 つ作り、0119-oracle と同じく、この評価の外では使わない。patch は `evals/experiments/call-depth/oracle-0120.patch` に置き、run には patch と binary の sha256 を残す。
  - `0120-oracle`: b57c0a376 に観測の変更を当てたもの。再帰しない経路の上限を 4,096 にし（再帰の経路の 128 は残す）、下の probe を足す。実装の前に build する。
  - `0120-oracle-after`: after に同じ変更を当てたもの（4,096 は評価の再帰の段と比べる）。実装の後に build する。
- **probe**（0119 の `probe_peak` を置き換える）:
  - 要求の始め（rtlc の `check` / `check_parallel`、core の `check-safety` / `check-tla`）に初期化し、終わりに読む。core は応答に `probe` の field を付けて返す（観測用の複製だけ。raw の比較では `probe` を除く）。driver は 1 つの process / instance に要求を 1 つずつ流すので、使い回しても要求ごとの値になる。並列の worker の値は、要求の中で atomic に足し合わせる。
  - 記録する値:
    - `peakDepth`: 256 と比べた値（評価の再帰の段）の最大。`0120-oracle` では before の深さ。
    - `peakTotal`: 128 と比べた値（合計）の最大。
    - `hits256`: product なら実際に 256 の拒否を返す判定の数。`descend` の 1 の v1 の早い return を通る判定と、2 の再帰の 128 が先に働く判定は数えない。`peakDepth` が 256 以上でも hit が 0 のことがある（例 A）。
    - `published256`: 256 の上限を持つ product なら、この要求で公開する応答。複製は 4,096 まで評価を続け、自分の実行から次のように計算する。
      - 逐次の検査: 最初の hit が起きたら、その位置の出力を応答とする。出力は、その位置を囲む fold のうち `replacement` が `Some` の最も内側のものの診断で、そのような fold が無ければ生の `Safety evaluation nesting exceeds 256`。hit が起きずに終われば、複製自身の応答を使う。
      - 並列の検査: 各 worker の event の列を、その worker の最初の hit の位置で切り、そこに hit の出力のエラーの event を置く。その列に、集約器と同じ公開の順（`safety_parallel.rs:146-167`。範囲の順に、候補を受け入れて違反を確定し、エラーの event で返る）を当てて、公開される応答を求める。
  - 記録は、判定（`descend`、途中の hop、葉の検査）の前に行う。再帰の 128 で拒否される位置も、その値で記録される。
  - fold の診断は、観測用の複製の thread-local な stack から取る。fold の本文を評価する間、その fold の `replacement` が `Some` のときだけ、それを積む（`FoldOrder::SequenceLeft` の fold は `None` なので積まない、`:1551-1557`）。本文のエラーは、`replacement` が `None` の fold では書き換えられずに外へ伝わる。fold の診断はどれも `Unsupported TLA input` で始まるので、一度書き換えられると外側の fold は書き換えない（`:1571-1580`）。そのため出力は、stack の一番上の診断になる。
  - 自己検査: `0120-oracle` の `published256` は、before の実際の応答と byte 単位で一致しなければならない。並列の検査で hit がある要求は（判定・エラーの種類と文言・trace で比べ、work は比べない）、予算が尽きる並列の検査は結果の種類とエラーの種類で比べる。これで、この計算が 256 の上限の効果（fold の書き換えと並列の公開の順を含む）を正しく再現していることを、実際の 256 の binary で確かめる。
  - stack の harness の `oracle-peak` の出力に、`peakDepth`・`peakTotal`・`hits256` と `published256` の要約を足す（feature の中だけ）。
- **故障版**: 人工故障で、検出を未知のバグの発見に数えない。実装の後に一時的に作って対照の感度を確かめ、結果を audit に残して戻す。
  - F1: hop で `forwarded` を増やさない（r1 の loop）。
  - F2a: 鎖ごとに `consume(1)` を 1 回だけ呼ぶ。
  - F2b: 先に鎖の参照をたどり、最後に `consume(n)` でまとめて課金する。
  - F3a: 終端の context を `prime` なしで作る。
  - F3b: 根が `Prime` の実引数も転送として素通りする。
  - F4: 終端の `locals`・`old` に使う側のものを使う。
  - F5a: ENABLED の subscript の context（`:1219-1228`）で `forwarded` を引き継がない。
  - F5b: ENABLED の step の context（`:1205-1214`）で `forwarded` を引き継がない。
  - F5c: `Snapshot` で `forwarded` を引き継がない。
  - F6: 途中の hop の判定を省き、終端でだけ判定する。
  - F7: `descend` で、256 の判定を v1 の早い return より前に置く。

### Red

テストを先に置き、before で意図した理由で失敗すること（Red）を記録する。

- before は plan の commit にテストだけを足したもの（product code は変えない）。
- stack overflow は test binary 全体を abort させるので、深い入力のテストは今と同じく、別の test target か大きさを指定した thread で動かす。
- テストの大きさは、上の規則から導く（256 と比べる値 = before の深さ − その位置までの途中の hop の数で、要求全体では位置ごとの最大）。書く前に `0120-oracle` の before の深さと出力で確かめる。
- 期待値の work と判定は `0120-oracle` から取る。hit が 0 の要求は before の出力、hit が 1 以上で after が実際の 256 の拒否に当たらない要求は `0120-oracle` の出力である。after が 256 の拒否に当たる要求の期待値は、規則と入力の作りから導き、実装の後に `0120-oracle-after` の `published256` で確かめる。拒否の位置と work は、規則と before の手順から導く。
- テストが意図した理由以外（検証のエラーなど）で失敗しないことを、before で確かめる。
- 実装の後の最初の実行で大きさが規則と違えば、0119 と同じく評価の前に plan に追記し、大きさを直して Red を取り直す。

`crates/rtlc/src/safety_definition_tests.rs`（深い入力は 8 MiB の thread）:

- **`v2_forwarded_arguments_do_not_add_evaluation_depth`（新）**: 再帰しない転送の鎖を、`evaluate_chain` と同じ形で Arena から評価する。形は `Parameter` だけ、`Capture` だけ、交互、無名の `Frame` を通るものの 4 つ。
  - `Parameter` だけの鎖の大きさ 254 / 255 / 256: after は 254 が `true`（work 508）、255 が最初の途中の hop の判定で拒否（work 256）、256 が使う側で拒否（work 256）。ほかの形の大きさは規則から導く。
  - main での意図した失敗: 254 が `Safety evaluation nesting exceeds 256`（work 256）で拒否される。255 と 256 は main と同じ出力（対照）。
- **`v2_forwarded_sets_and_functions_do_not_add_evaluation_depth`（新）**: `MembershipSet::eval` の経路（`sequence-in` の右辺を転送の鎖の先に置く）と、`eval_function` の経路（`at` の target を転送の鎖の先に置く）。main での意図した失敗: 受理される大きさが 256 で拒否される。
- **`v2_refusal_moves_to_the_deepest_point_after_forwarding`（新）**: Arena から評価する `And(forward(200), call(300))`（右は引数のない call の鎖で、定義は左と別）。
  - after は左が complete、右の call の 255 番目の判定で拒否。規則からの予測は work 400 + 255 = 655。
  - main での意図した失敗: 左の途中の hop で拒否される（予測は work 255）。
  - 両辺が真の `And(forward(100), call(198))` は hit が 0 で、before・after とも同じ work で complete になる（対照）。要求全体の最大の取り方は、下の sweep の `and-forward` の 3 入力で確かめる。
- **`v2_refusal_continuation_enters_recursion_after_forwarding`（新、例 A）**: Arena から評価する `forward` の鎖で、最も外側の実引数を、本文が `TRUE` の再帰する `R()` にする。
  - 254 段: after は `evaluation-budget` の `Recursive safety evaluation nesting exceeds 128`、work 509（`0120-oracle` の出力）。本文の入口で評価の再帰の段は 256 になるが、再帰の 128 が先に働く。
  - 128 段: after は同じ `evaluation-budget`、work 257。
  - main での意図した失敗: どちらも work 256 で `Safety evaluation nesting exceeds 256`。
  - 256 の判定を再帰の 128 より先に置く実装では、254 段が protocol の拒否になって失敗する。
- **`v2_applied_lazy_function_body_counts_the_depth_of_its_application`（既存、期待値を変える）**:
  - 70 段の転送と 130 段の本文の組み合わせを complete にする。
  - 目的（遅延関数の本文は適用した場所の深さから数える）を保つため、after でも合わせると 256 を超え、片方だけでは収まる組で拒否を検査する（転送 P 段と本文 B 段、P と B は規則から導く）。
  - main での意図した失敗: 70 / 130 が拒否される。
- **`v2_enabled_subscripts_resolve_forwarded_parameters_without_depth`（新）**: 引数を持つ再帰しない定義の本文の ENABLED で、subscript が引数を k 段の転送の先で読む。step の数と k は、before の深さが 256 以上、after の値が 256 未満になる組を規則から導く。main での意図した失敗: `Safety evaluation nesting exceeds 256`。
- **`v2_primed_forwarding_keeps_the_primed_values`（新）**: x を変える代入の後の Next の guard `F_k(x)' = F_k(Prime(Read x))`。左は使う側の prime、右は手で置いた終端の `Prime` で、`F_k` は k 段の転送。
  - 浅い k は before でも通る対照、深い k は main で 256 の拒否になる Red。
  - guard は、使う側の prime が鎖を通って保たれ、終端の `Prime` が次の状態を読むときだけ真になる。F3a は左で、F3b は右で、生成数を変えて失敗する。
- **`v2_fold_bodies_resolve_forwarded_elements_without_depth`（新）**: 文字列の集合 `{"a", "b"}` の fold で、本文を `acc ∪ {F_k(element)}` にする（`F_k` は k 段の恒等の転送）。
  - k = 200: after は complete（`0120-oracle` と同じ）。main での意図した失敗: fold の本文の中の深さの拒否が、文字列の fold の診断に書き換えられて返る。
  - k = 300: before・after とも同じ文字列の fold の診断（対照。after も同じ fold の本文の中で 256 に達する）。
- **`v2_fold_rewrites_follow_the_first_refusal_after_forwarding`（新、r2 の指摘 6 の入力）**: 状態 0・1・2 を順に訪れる program で、invariant が状態ごとに同じ形の union の fold を評価する。
  - 状態ごとの fold: 状態 0 は文字列の集合で本文が `F200(element)`、状態 1 はレコードの集合で `F300(element)`、状態 2 は文字列の集合で `1 \div 0`。
  - main: 状態 0 の深さの拒否を、文字列の fold の診断に書き換えて返す。
  - 上限 4,096 の `0120-oracle`: 状態 2 の 0 除算を、同じ診断に書き換えて返す。raw の応答は main と同じだが、hit は 1 以上。
  - after: 状態 0 を通り、状態 1 で 256 に達して、レコードの fold の診断を返す（`0120-oracle-after` の最初の hit の出力と同じ）。
  - main での意図した失敗: 文字列の fold の診断が返る。
- **`v2_fold_rewrites_skip_folds_without_a_replacement`（新、r3 の指摘 6）**: 外側を文字列の集合 `{"a", "b"}` の union の fold、内側を要素 1 個の sequence の fold にし、内側の本文で `F_k(element)` を読む。内側の fold の `replacement` は `None` なので、内側で起きた拒否は書き換えられずに外へ伝わり、外側で文字列の fold の診断に書き換えられる。
  - k = 300: before・after とも文字列の fold の診断（対照）。`0120-oracle-after` の `published256` も同じ文言になり、probe の stack が `None` の fold を積まないことを確かめる。
  - k = 200: after は complete（`0120-oracle` と同じ）。main での意図した失敗: 文字列の fold の診断が返る。
- **`v2_forwarding_hops_charge_one_unit_of_work_each`（新、before でも通る対照）**: 6 hop の鎖を、`Arena::new(m)` の m = 0〜W + 1 のすべてで評価する。m < W は `budget` で `arena.work == m`、m ≥ W は `true` で work W。F2a で失敗する（F2b はここでは区別できない）。
- **`v2_forwarding_charges_each_hop_before_the_next_reference`（新、対照）**: `forward` の 6 段の鎖の 3 番目の参照の slot を範囲外にする。
  - `Arena::new(7)` では、2 回目の引数の要求で `budget`（work 7）。F2b は `Invalid lexical reference` を返して失敗する。
  - 十分な予算では、3 番目の参照で `Invalid lexical reference`（work 8）。
  - どちらも main と同じ出力。
- **`v2_forwarding_refuses_before_the_next_reference`（新、対照）**: どれも main と同じ出力になる。F6 では順に、work 切れ、`Invalid lexical reference`、work 510 に変わって失敗する。
  - `forward(255)` を `Arena::new(256)` で評価すると、work 切れではなく `Safety evaluation nesting exceeds 256`（work 256）。
  - `forward(255)` の 2 番目の参照の slot を範囲外にすると、`Invalid lexical reference` ではなく同じ深さの拒否。
  - 十分な予算の `forward(255)` は、work 256 で同じ拒否。
- **`v2_recursive_forwarding_keeps_the_former_limit`（新、対照）**: `call_chain` と同じく再帰の印を付けた転送の鎖の、大きさ 63 / 64 / 65。63 は `true`（work 126）。64 と 65 は `evaluation-budget` の `Recursive safety evaluation nesting exceeds 128`（work は before の値で、規則では両方 128）。F1 で 64 が complete になって失敗する。
- **`v2_recursion_entered_after_forwarding_keeps_the_former_limit`（新、対照）**: 再帰しない `forward` の 64 段の鎖の最も外側の実引数を、本文が `TRUE` の再帰する `R()` にする。hit は 0 で、before・after とも `evaluation-budget`・work 129。F1 で complete になって失敗する。
- **`v2_forwarded_depth_reaches_enabled_and_snapshots`（新、対照）**: 再帰しない `forward` の 60 段の鎖の終端から、再帰する定義に入る 3 つの入力。どれも hit は 0 で、before と同じ `bounded / evaluation` と work になる。鎖の終端で評価の再帰の段は 61、転送の段は 59 で、合計は 120。F1 は 3 つとも、F5a〜F5c はそれぞれ 1 つだけを complete にして失敗する。
  - subscript（F5a）: `R()` の本文が、5 step と literal の subscript を持つ ENABLED。`R` と ENABLED に入った後の合計は 122 で、step の訪問は 127 までに収まり、subscript の入口だけが 122 + 5 + 1 = 128 になる。
  - step（F5b）: `R()` の本文が、subscript の無い 10 step の ENABLED。6 番目の step の訪問で合計が 128 になる。
  - snapshot（F5c）: 鎖の終端が `At(Function{k ∈ {1} ↦ R_0()}, 1)`。`R_0`〜`R_9` は再帰の印を付けた引数のない call の鎖で、最後は `TRUE`。snapshot の合計は 122 で、`select` が束縛の確認と本文への移行でそれぞれ 1 回 `descend` するので、`R_0` の入口は 124 になる。最初に 128 になるのは、`R_3` の本文で `R_4` を評価する位置である。そこまでの課金は、鎖の 60 call、引数の要求 60 回、束縛の tick 1 回、`R_0`〜`R_3` の call 4 回で、規則からの予測は work 125（`{1}` の domain の評価が work を使えば、その分を足す）。書く前に `0120-oracle` の値で確かめる。
- **`v2_enabled_subscripts_keep_the_recursive_limit_for_forwarded_parameters`（新、対照）**: 上の ENABLED の forwarded subscript の入力に再帰の印を付け、合計が 128 をまたぐ組で、before と同じ `bounded / evaluation` になる。転送は subscript の中で起きるので、F1 で失敗する。
- **`v1_enabled_subscripts_at_256_skip_the_depth_check`（新、対照、`crates/rtlc/src/safety_expressions.rs` の test module）**: 既存の `v1_enabled_subscript_keeps_the_existing_long_branch_limit` と同じ v1 の ENABLED で、step を自己代入 1 個と真の guard 254 個にする。subscript に渡す見積もりは 255 + 1 = 256 で、今は `descend` の v1 の早い return を通り、`false` を返す。before・after とも `false`。F7 で protocol の拒否になって失敗する。
- **`v2_forwarding_errors_keep_their_messages_and_work`（新、対照）**: Arena から評価する壊れた IR。範囲外の slot を k hop 先に置くと `Invalid lexical reference`、frame の外の call の引数を転送すると `Missing lexical call frame` で、どちらも work は before の値。
- **`v2_forwarded_bound_and_old_keep_their_lexical_values`（新、対照）**: 終端が量化の束縛（`\A y \in S : F_k(y)`）の入力と、EXCEPT の `@`（capture の鎖）の入力が、before と同じ値になる。F4 で失敗する。

stack と境界のテスト:

- **`crates/rtlc/tests/stack_programs/mod.rs`**: 転送を含む 4 種類を足す。
  - `capture`: 定義 k は capture 1 個、本文は `Call(k + 1, captures: [Capture 0])`、最後は `Capture 0`、根は `Call(0, captures: [true])`。
  - `mixed`: 偶数の定義は `Parameter` を capture として、奇数の定義は `Capture` を引数として次へ渡す。
  - `forward-set`: `Inv == sequence-in(<<>>, Call(0, [sequence-set({true})]))`、定義は `forward` と同じ転送。
  - `forward-function`: `Inv == at(Call(0, [function k ∈ {1} ↦ x]), 1) >= 0`。
- **`crates/rtlc/tests/evaluation_stack.rs`**: `forward` を 254 / 255（work 1,020）にし、4 種類の受理の最大と次の大きさを足す（release は 2 MiB、debug は 8 MiB の thread）。main での意図した失敗: 受理側が `Safety evaluation nesting exceeds 256` で拒否される。ほかの 12 種類は変えない（対照）。
- **`crates/rtlc/tests/parallel_stack.rs`**:
  - 転送の境界: 32 個の初期状態を持ち、Next で `counter' = Call(0, [counter])`（定義は `forward` と同じ転送）を代入するモデルを、worker 2 の `check_parallel` と `check` で検査する。after で受理される最大の大きさで、両方が complete 32 / 64 になり、work が `0120-oracle` の同じ検査の値と一致する。次の大きさは型付きの拒否（全 worker の最初の hit が同じ生の拒否になる入力）。main での意図した失敗: 受理側が 256 で拒否される。
  - 例 B（新）: 初期状態 `x ∈ 0..31`、worker 2、`maxWork` 10,000,000。Next は、`x = 0` で停止しない再帰（`Loop(v) == Loop(v)` を代入の値に使う）、ほかの状態で `x' = forward200(x)`。公開の結果は `bounded / evaluation` で、work は `0120-oracle` の同じ `check_parallel` の値と一致する。`x = 0` を持たない worker は、範囲の分け方によらず、before では最初の親状態で 256 に当たる。main での意図した失敗: `x = 0` を持たない worker が最初の親状態の 256 で止まり、work が少ない。
  - 例 C（新）: 初期状態 `x ∈ 0..15`、worker 2、`maxWork` 10,000,000、`Inv == x < 16`。Next は、`x = 0` なら `x' = forward200(x) + 16`、ほかの状態は `x' = forward300(x)`。公開の結果は invariant の違反（trace は `0`、`16` の 2 状態）で、`0120-oracle-after` の `published256` と一致する（判定・エラーの種類と文言・trace で比べ、work は比べない）。main での意図した失敗: `x = 0` の 256 の protocol の拒否が返る。
  - membership のテストは変えない。
- **`crates/rtla-compiler/tests/call_depth.rs`**:
  - `layered_calls_complete_up_to_the_nesting_limit` を N ∈ {40, 41, 76, 83, 84, 85, 124, 125}（期待値は上の予測）にし、`layered_calls_past_the_nesting_limit_are_refused` を N ∈ {126, 168, 169} にする。
  - 下の 5 つの族の受理の最大と最初の拒否を足す。
  - main での意図した失敗: N ∈ {84, 85, 124, 125} と族の受理側が 256 で拒否される。
  - `generated_layered_calls_complete` は変えない（対照）。

4 経路・host・TLC のテスト:

- **`tests/call-depth/contract-test.ts`**: `fixtures/call-depth/records-0120.json` と `cases-0120.json`（`evals/experiments/call-depth/expectations-0120.ts` が導く）を読み、4 経路 × v1 / v2 で検査する。
  - 境界の検査: hop の 125 / 126 は Rust の 2 経路で、hop-invariant の 124 / 125 は 4 経路で行う。
  - 拒否の後の instance の検査: hop-invariant-125 の拒否の後に hop-invariant-40 を流す。
  - main での意図した失敗: 新しく受理されるセルが 256 で拒否される。
- **`runCallDepth`**（`tests/wasm-host/contract.ts`、`tests/portable-browser-entry.ts`、`tests/portable.browser.spec.ts`）: N = 125 の complete 5 / 6・work 1,282、N = 126 の型付きの拒否、拒否の後の N = 40 の complete 5 / 6・work 432。main の core での意図した失敗: N = 125 が拒否される。
- **TLC との比較**: 新しい入力の TLC の verdict と違反の trace は rtlc に依らないので、実装の前に `0120-tlc` として取る。
  - `expect-0120.ts` は、TLC と比べられるセルで、次が `0120-tlc` と一致しなければ fixture を書かない。
    - 期待が complete のセル: status、状態数、生成数。
    - 期待が違反のセル: 状態数、生成数、違反の trace の各状態の変数の値。
    - 期待がエラーのセル: エラーの種類だけ（rtlc のエラーの応答は `code` と `message` だけで、状態数・生成数を持たない）。例えば `division-by-zero-120` は、TLC の 0 除算と rtlc の `Integer division by zero` の対応を比べる。
  - 境界の受理側は、この比較を通った期待値で main に対して Red になる。

### 期待値が変わる既存のテスト

- 期待値が変わる既存のテストは、次だけとする: 上で「既存、期待値を変える」と書いたもの、`evaluation_stack.rs` の `forward`、`call_depth.rs` の 2 つ、`tests/call-depth/contract-test.ts`、`runCallDepth` の 3 file。
- `fixtures/` の既存のファイルは変えない（`fixtures/nesting-chains/` の全ファイルと、`fixtures/call-depth/cases-0119.json`・`records-0119.json`・`chain_N50.tla` を含む）。
- `crates/rtla-compiler/tests/nesting_chains.rs` と `tests/nesting-chains/contract-test.ts` は、今と同じく `cases-0119.json` の `calls-127〜129` だけを読む。この 3 つは転送を含まないので変わらない。
- 実装の前に、debug と release の全テストを流して分類を確かめる。分類と違うテストが失敗したら、実装を直すか、評価の前に条件の変更として plan に追記する。

### 入力

合成の入力は公開用に作ったもので、特定の対象の名前を含まない。乱択は使わない。0119 の追記のとおり、定数の引数は評価の前に畳まれるので、引数は状態に依存させる。`evals/experiments/call-depth/sources-0120.ts` が作り、0119 の `sources.ts` と `expectations-0119.ts` は記録として変えない。

- 0119 の 28 入力と `calls-127〜129` を、綴りを変えずにそのまま含める。
- hop と hop-invariant の N ∈ {124, 125, 126}。
- 転送の族。どれも N ∈ {10, b, a, a + 1} で、b は main で受理される最大、a は after で受理される最大（規則から導き、`0120-oracle` の before の深さで確かめる）。TS frontend でも compile できるように `%` と `\div` を使わない。
  - `record-hop-N`: `VARIABLE state`、`Init == state = [count |-> 0]`、`rec0(s) == [s EXCEPT !.count = IF s.count < 4 THEN s.count + 1 ELSE 0]`、`recK(s) == rec(K-1)(s)`、`Next == state' = recN(state)`、`Inv == state.count >= 0`。レコードの状態を、受け取った引数をそのまま渡す層に通す形（手元のモデルの形）。層あたり before 2 段、after 1 段の見込み。
  - `let-hop-N`: `lhop0(x) == x + 1`、`lhopK(x) == LET inner == lhop(K-1)(x) + 1 IN inner`、`inv == lhopN(counter) >= 0`（Next は hop-invariant と同じ walk）。LET の定義が外側の引数を capture し、次の operator に渡すので、`Capture` と `Parameter` が交互に並ぶ。
  - `set-hop-N`: `shop0(S) == S`、`shopK(S) == shop(K-1)(S)`、`Inv == counter \in shopN(0..4)`（membership の evaluator、Next は walk）。
  - `function-hop-N`: `fhop0(f) == f[1]`、`fhopK(f) == fhop(K-1)(f)`、`Inv == fhopN([k \in {1} |-> counter]) >= 0`（関数の evaluator、Next は walk）。
  - `primed-hop-N`: `fwd0(x) == x`、`fwdK(x) == fwd(K-1)(x)`、`Next == counter' = (IF counter < 4 THEN counter + 1 ELSE 0) /\ fwdN(counter)' = fwdN((counter + 0)')`。左は使う側の prime、右は根に `Prime` が残る終端。`expect-0120.ts` は、両 frontend の compile した IR で右辺の実引数の根が `prime` であることを確かめ、そうでなければ、その frontend のセルを記録した理由とともに外す。
- 要求全体の最大の対照（`and-forward`、Next は walk）: `fwd0(y) == y >= 0`、`fwdK(y) == fwd(K-1)(y)`、`c0 == counter >= 0`、`cK == c(K-1)`。左は転送の鎖、右は引数のない call の鎖で、どちらも状態を読むので評価の前に畳まれない。3 入力とも外側の `And` を持ち、片側の対照のもう一方の項は浅い `counter >= 0` にする。
  - `Inv == fwd100(counter) /\ c198`。
  - `Inv == fwd100(counter) /\ counter >= 0`。
  - `Inv == counter >= 0 /\ c198`。
- 意味の対照:
  - `division-by-zero-120`: 最も内側の本文の `x \div 0`（0119 の 70 と同じ綴り、wrapper つき、after の深さ 5 + 2N = 245）。
  - `violation-120`: `inv == hop120(counter) < 124` の違反（hop-invariant と同じ形で、after の深さ 6 + 2N = 246、trace は 4 状態の見込み）。
- 再帰の対照: `recursion-from-hop-50` のほかに、hop の段の数を変えた `recursion-from-hop-M` を 1 つ足す。
  - M は、テストを書く前に `0120-oracle` の記録と規則から選ぶ。条件は、`0120-oracle` で hit が 0 の `bounded / evaluation` になり、F1（転送の段を数えない）では合計が 128 未満に収まること。
  - 実装の後に、F1 で結果か work が変わることを確かめる。変わらなければ評価の前に追記し、TLA+ の対照は `recursion-from-hop-50` だけにして、F1 の区別は IR の unit test に任せる。

### 期待値と許す差

- **要求の分け方**: どのセルも、`0120-oracle` の `hits256` で分ける（上の「変えないものと、変わってよいもの」）。raw の応答の差では分けない。3 状態の fold の入力のように、256 が結果を決めても raw の応答が同じになる場合があるからである。
- **期待値の導き方**（`expectations-0120.ts`）: `cases-0120.json` の各入力は、after の 256 と比べる値の最大 `depth` と、before の深さ `formerDepth` を持つ。
  - v1 は before の出力。
  - 各入力は、after が実際の 256 の拒否に当たるかどうか（`afterRefuses`）も持つ。規則から導き、`peakDepth` ではなく、`descend` の判定の順（再帰の 128 が先）を当てて決める。実装の後に `0120-oracle-after` の `hits256` が 0 か 1 以上かと一致することを確かめる。
  - v2 は、`0120-oracle` の hit が 0 なら before の出力。
  - hit が 1 以上で、`afterRefuses` が偽なら `0120-oracle` の出力。2 つの複製は合計の深さが 4,096 未満で同じ出力になるので、実装の前はこれを after の期待値に使い、実装の後に `0120-oracle-after` の `published256`（このとき複製自身の応答）との一致で確かめる。
  - hit が 1 以上で、`afterRefuses` が真なら、型付きの拒否か、並列で先に確定する結果。入力の作りから宣言する（fold の外なら `Safety evaluation nesting exceeds 256`、fold の本文の中なら、その位置を囲む fold のうち `replacement` が `Some` の最も内側のものの診断、並列なら集約器の公開の順の結果）。実装の後に `0120-oracle-after` の `published256` と一致することを確かめる。
  - `formerDepth` は `0120-oracle` の `peakDepth`、`depth` は `0120-oracle-after` の `peakDepth` と一致することを `0120-oracle` に残す（一致の判定は下の sweep の規則による）。
- **結果の期待値**: status・状態数・生成数・違反の trace は固定 TLC から取る。
  - TLC は 0119 と同じ v1.7.4（revision 5a47802b、jar の sha256 936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88）を、`-Xmx256m -Xss8m -workers 1` で 1 つずつ実行し、scratch と metadir をケースごとに分ける。
  - 0074・0075・0119 の入力は、入力の sha256 が一致し、比べる項目がすべて記録されているものだけ、[0074-tlc](../runs/0074-tlc/)、[0075-tlc](../runs/0075-tlc/)、[0119-tlc](../runs/0119-tlc/) の結果を引用する。trace の状態を保存していない違反のケース（`violation-76` は `states`・`generated`・`trace` の数だけ）は、固定 TLC で取り直して `0120-tlc` に保存する。新しい入力はすべて実行する。
  - work とエラーの文言の比較先は、セルの種類で決める。
    - `0120-oracle` の hit が 0 のセル: before の raw の応答（`0120-oracle` も同じ）。
    - `0120-oracle` の hit が 1 以上で、after が 256 の拒否に当たらないセル: after と複製は同じ意味の結果（同じエラー、同じ違反、`bounded / evaluation` など）に達するので、複製の応答（`0120-oracle-after` の `published256` は、このとき複製自身の応答と同じ）と比べる。
    - after が 256 で拒否するセル: 複製は 4,096 まで続けて complete や別の結果になりうるので（固定した `hop-168` は 0119 でも、複製が complete・after が拒否の組み合わせだった）、複製自身の応答とは比べず、`0120-oracle-after` の `published256` と比べる。
- **許す差**: `0120-oracle` の hit が 1 以上のセルだけが変わってよい。
  - 変わった先は、`0120-oracle-after` の `published256` と同じでなければならない。逐次の検査は raw の応答をすべて比べる。並列の検査で hit がある要求は（判定・エラーの種類と文言・trace で比べ、work は比べない）、予算が尽きる並列の検査は結果の種類とエラーの種類で比べる。`0120-oracle-after` の hit が 0 のセルでは、TLC と比べられる判定は固定 TLC とも一致しなければならない。
  - 該当するセルを一覧にして保存し、そのうち before の文言が fold の診断のものの数も残す（既存の入力での予測は 0）。
  - hit が 0 のセルの差は 0 件を要求する（v1、再帰の `bounded / evaluation` の結果と work、work 切れ、エラーの種類と文言、予算が尽きない並列の検査を含む）。予算が尽きる並列の検査は、上の結果の種類の規則による。
  - `0120-oracle` と `0120-oracle-after` の `published256` の自己検査（before と after の実際の応答との一致）を、全セルで求める。
- **2 つの観測用の複製の照合**: 下の境界の sweep と 4 経路の全セルで、次を要求する。
  - `0120-oracle-after` の raw の応答（`probe` を除く）が、`0120-oracle` と byte 単位で一致する。比べるのは status・状態数・生成数・work・違反の trace・エラーの code と文言。
  - `0120-oracle-after` の `peakTotal` が、`0120-oracle` の `peakTotal`（= before の深さ）と一致する。
  - どのセルでも、`0120-oracle-after` の `peakDepth` は `peakTotal` 以下で、`hits256` は `0120-oracle` 以下になる。
  - 転送を含まない入力では、`0120-oracle-after` の `peakDepth` と `peakTotal` が等しく、`hits256` が `0120-oracle` と等しい。
  - `0120-oracle` の `peakTotal` が 4,096 に当たるセルは、この照合から外して記録する（0120 の入力にはない見込み）。

### 経路

- 0119 と同じ 4 経路（TS frontend から Wasm、TS frontend から native、Rust frontend の Wasm、Rust frontend の native）と v1 / v2 で、before・after・2 つの観測用の複製を流す。0074 の 265、0075 の 81、0119 の 32 の入力に、0120 の入力を加える。
- 4 経路の driver（`evals/experiments/call-depth/paths-0120.ts`）と解析（`evals/experiments/call-depth/analyze-0120.ts`）を、experiments に置く。0119 の driver（`evals/runs/0119-audit/drivers/paths.mjs`）は記録として変えない。
  - 0119 の driver と `evals/experiments/nesting-chains/outcome.ts` の正規化は、エラーの `code` を捨て、文言の接頭辞と位置を除き、違反の trace を長さに縮める。
  - 0120 の driver は、正規化した outcome（fixture の比較用）に加えて、各セルの raw の応答の sha256（エラーの code と文言、trace の全状態を含み、`probe` を除く）、違反の trace の全状態、観測用の複製の `probe` を保存する。
  - hit が 0 のセルは、before と after の raw の応答の一致を要求する。許す差のセルで after が `0120-oracle-after` と一致すべきものも、raw の応答で比べる。
  - outcome.ts は fixture が使うので変えない。
- 同じ frontend の native と Wasm の結果の一致を要求する（0119 と同じ 5 セルの decoder の差を除く）。
- frontend を変えないので、両 frontend の IR は before と after で byte 単位で同じになることも要求する。
- TS と Rust の正規化した IR は、両方が compile できるセルで今と同じく比べる。0120 の新しい入力で違いが出たら、frontend の差として記録する（この変更の差には数えない）。
- 並列（`check_parallel`、worker 2）、CLI、Go（wazero の直接の呼び出し）、Chromium は下の項で別に確かめる。4 経路の要求（`check-safety`・`check-tla`）は逐次の検査である。

### 測定

- **既存の入力の結果**: 完全応答の `evaluator-corpus`（Quint6・TCommit・共有 chain、8 経路・各 4 iteration）と、旧版 156 モデル・1,560 要求の `just temporal-parity` を before / after で流し、program・結果・work の byte 単位の一致を要求する（記録した `bounded` を含む）。どちらも hit が 0 の予測である。差が出た要求は、`0120-oracle` で hit を数え、0 なら不合格、1 以上なら上の許す差の規則で判断する。
- **境界の sweep**:
  - 実装の前に `0120-oracle` で次を流し、probe の値と出力を残す: stack の harness の `hop` の N = 1〜200、hop-invariant の N = 1〜200（`source`）、`forward`・`capture`・`mixed`・`forward-set`・`forward-function` の大きさ 1〜300、ほかの 12 種類の 0119 の境界の大きさ ± 1、5 つの族の N = 1〜300、`and-forward` の 3 入力。harness は 1 回の起動で 1 つの要求を流すが、probe は要求の始めに初期化する。
  - 構造上の深さが 128 以上の点で、`peakDepth`（`0120-oracle` では before の深さ）が宣言（hop は 5 + 3N、hop-invariant は 6 + 3N、`forward` は 2s）と一致することを確かめ、テストの大きさを決める（`boundaries-255.json`）。128 未満の点は記録を残し、宣言との差（葉が判定を呼ばないことによる 1）を一致の条件に含めない。
  - 実装の後に `0120-oracle-after` で同じ点を流し、上の照合を確かめる。`peakTotal` は全点で `0120-oracle` と一致することを求める。
  - `peakDepth` は、構造上の値（hop は 5 + 2N、hop-invariant は 6 + 2N、`forward` は s + 1）が 128 以上の点で一致し、受理の最大と次の大きさが全族・全種類で予測と一致することを求める。
  - `and-forward` は、片側の 2 入力の `peakDepth` の大きい方が、両辺をつないだ入力の `peakDepth` と一致することを求める。3 入力とも外側の `And` を持つので、片側の入力の深さは結合した入力の同じ側と同じになる。
- **stack**: [0119 の harness](../experiments/call-depth/stack-harness/) と driver（`stack.ts`）を使い（種類は `stack_programs` から読む）、before・after・`0120-oracle-after` の debug と release で流す。
  - セル: 17 種類の、after で受理される最大の大きさ（`boundaries-255.json`）と、256 と比べる値が約 125 の大きさ。hop は N = 60・122・125（深さ 125・249・255）。
  - 1 GiB の出力を基準に 4 KiB の分解能で最小の stack を二分探索し、0119 と同じく、セルごとに終了コード・signal・stdout・stderr を保存する。
  - 条件（0119 と同じ）: 受理される最大の大きさでの最小の stack が、release で 1,024 KiB 以下、debug で 6,144 KiB 以下。
  - 転送を含まない 12 種類は、before と after の差も記録する。
- **Wasm**: `evals/experiments/call-depth/wasm-ceiling.ts` と `browser-ceiling.ts` を使い（セルは引数で渡す）、`0120-oracle-after` と after の core を要求ごとに新しい instance で動かす。
  - host: Node v24.22.3 の既定・`--liftoff-only`・`--no-liftoff`（別の process）と、Playwright の Chromium の main thread と dedicated Worker。
  - セル: 17 種類と、hop の IR（`check-safety`）と `check-tla` の 19 個。探索範囲は hop が N 1〜2,000、17 種類が大きさ 1〜4,000。転送を含まない 12 種類も、loop を置く `eval_reference` をすべての call が通るので測り直す。
  - driver の引数: 両 driver は、要求を作るときに `kindBounds.maxWork = 1,000,000` を固定し、`complete` と `bounded` を受理に数える（`wasm-ceiling.ts` の `accepted`）。0120 では両方に次の 2 つの引数を足し、既定の挙動は今と同じに残す。
    - 受理の判定: `complete` だけを受理とする。
    - `maxWork`: 要求の work の上限。
  - 再試行: 上端の候補より小さい大きさで `bounded` が出たセルは、`maxWork` 100,000,000 で 1 回だけ流し直す。それでも `bounded` なら、そのセルの上端を「最後に complete になった大きさの値以上」の下限として扱い、その下限で条件を判断する。
  - 観測用の複製の上限 4,096 が先に来るセルは、その host の上端を、4,096 の直前の値以上として扱う。
  - 条件: 0119 の規則（上限 L に対して上端が 2L 以上）を L = 256 のまま適用する。
    - どの host でも、`0120-oracle-after` の上端（256 と比べる値）の最小が 512 以上。
    - after が全セルで、`boundaries-255.json` の受理の最大の大きさを complete で受理し、次の大きさを型付きで拒否する（`RuntimeError` と `RangeError` は 0 件）。
  - 拒否の後も同じ instance で次の要求が complete になることを、Node の host と Chromium の `runCallDepth` で確かめる。
  - Go: `evals/experiments/call-depth/go/main.go` は `adapters/go` を通らず、wazero v1.12.0（go@1.25.14）で core の Wasm を直接 instantiate して、入力ごとの応答を 1 行ずつ表示する。trap しても `trap: …` を表示し、終了コード 0 で終わる。
    - 確かめる範囲は wazero の直接の呼び出しまでとし、`adapters/go` の経路の深さは測らない（adapter は `just go-test` で別に検査する）。
    - after で hop の 125 / 126 / 168 / 169 を流す。`analyze-0120.ts` が各入力の行を期待値と照合する（125 は `complete 5 6 1282`、126・168・169 は `error protocol: Safety evaluation nesting exceeds 256`）。
    - `trap:` の行は失敗とし、終了コードでは判定しない。
    - `0120-oracle-after` で hop の上端を 1 回流し、上端は最後に complete の行が出た N とする。
  - before の参照値は [0119-wasm-ceiling](../runs/0119-wasm-ceiling/) の `oracle-r2` を引用し、測り直さない（0119-oracle と `0120-oracle` の違いは、ENABLED の subscript の 1 行と probe だけで、stack の種類に関係しない）。
- **CLI**: `just cli-wasm-build` で after から wasm32-wasip1 の CLI を build し直し、`evals/experiments/call-depth/cli-ceiling.ts` を 0119-cli-r2 と同じ範囲（`hop:1:300 operator:1:300`）で流す。CLI は v1 しか出さず、v1 は `Parameter` を持たないので、結果は 0119-cli-r2 と同じになることを要求する（対照）。違えば評価を止め、原因を調べる。
- **Criterion**: 0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で、0119 と同じ driver・14 関数の kernel（`just evaluator-kernel`）を流す。
  - frontend は変えないので、rtla-compiler の bench は流さない。
  - 10% 以上の後退は同条件の ABBA で再確認し、再現する後退を残したまま採用しない。
  - `size_of::<EvalContext>()` を before と after で記録する。

### 手順と実行の条件

- plan は結果を見る前に commit する。実装とテストを書く中で分かった条件の変更は、評価を流す前に「実装の段で追記した条件」として追記し、上の記述は変えない（0119 と同じ）。
- この host での build・test・測定・benchmark は 1 つずつ流し、同時に流さない。
- **RSS と時間の上限**: 測定と検査の command は、RSS の上限 4 GiB の watchdog（1 秒ごとに process の木の RSS を読む）の下で動かす。Chromium も同じ。
  - 超えたら kill して `rss-cap` として記録する。
  - wall clock の上限は次のとおり。超えたら `timeout` として記録する。
    - TLC の 1 ケース: 300 秒。
    - sweep・stack の探索・Wasm の上端の探索の 1 要求: 120 秒。
    - それ以外の 1 command: 30 分（0119 の audit で最も長い command は 134 秒）。
  - `rss-cap` と `timeout` は合格に数えず、同じ ID の run を上書きしない。
- **予算**: idealis の要求の状態数と work の上限は、各 experiment が記録した値をそのまま使う。
  - fixture と 4 経路は 0119 と同じ `bounds`。`stack_programs` は 100・1,000,000、harness は 10,000・100,000,000、Wasm の上端の探索は 0119 と同じ `hopBounds` と `kindBounds`。
  - `evaluator-corpus` と `temporal-parity` は、それぞれの plan が記録した要求ごとの上限。
- **既存の入力**: 記録した出力との一致を条件にする。56 状態の `LongIfChain-safety`、6,816 状態の TCommit-8、`maxWork = 1` で `bounded / work` を返す `temporal-parity` の対照を含め、`bounded` も記録どおりなら一致とする。
- **0120 の新しい入力**: 到達する状態を 32 以下に作る。complete を期待したセルが `bounded`（states / work）になったら、そのセルの条件を満たさなかったものとして扱い、合格にしない。
- TLC は `-Xmx256m -workers 1` で流す。

手元のモデルは公開しない。手元で after と `0120-oracle-after` を流し、0119 の plan に記録した 31 件の検査要求について、次だけを iteration に記録する（名前・パス・内容は記録しない）: 256 で止まるもの、通るもの、`bounded` になるものの件数、観測用の複製（before）で hit が 1 以上になる件数、観測用の複製（after）で測った `peakDepth` と `peakTotal` の最大。この記録は採否の条件に含めない。

## 記録と完了条件

run は追記のみ。`evals/runs/0120-*` に、source / binary / 入力・実行順・予算・host・終了コード・`rss-cap` と `timeout` を保存し、失敗も残す。driver と解析のコードは `evals/experiments/call-depth/` に置き、run には実行した command と、その時点の code の sha256 を残す。README の規則に従い、commit するのは iteration が引用する summary（表、集計、manifest、終了コード、sha256）だけにする。Criterion の sample、ケースごとの出力、log、大きな dump は、消した数・バイト数・digest を `reduced.json` に残す。記録する時点で、実行したホストの絶対パスとユーザー名を placeholder に置き換える。

予定する run:

- `0120-tlc`（新しい入力と、trace の状態を取り直す引用元のケース）。
- `0120-oracle`（2 つの複製の patch・sha256、境界の sweep、`boundaries-255.json`、深さの宣言の照合、要求ごとの probe）。
- `0120-paths`（raw の応答の digest、trace、probe を含む）。
- `0120-corpus` / `0120-corpus-summary`、`0120-temporal-parity`。
- `0120-safety`（`just safety-eval` と `just safety-replay`）。
- `0120-stack`、`0120-wasm-ceiling`、`0120-cli`。
- `0120-kernel` / `0120-kernel-summary`、必要なら `0120-abba-N`。
- `0120-browser`、`0120-evaluate`、`0120-loop`。
- `0120-audit`（Red・Green・故障版 F1〜F7 の記録と、`published256` の自己検査を含む）。

終了前に次を実行する。

- `just check`、`just wasm-test`、`just safety-test`、`just rtlc-test`、`just compiler-test`、`just quint-test`、`just portable-build` / `just portable-test`、`just go-test`。
- `just safety-eval 0120-safety` と `just safety-replay 0120-safety`。TLC の意味の照合として、固定した jar で `just safety-tlc` を 1 回流す。
- 深い入力のテストを release でも流す: `cargo test --release --locked --manifest-path crates/rtlc/Cargo.toml --test evaluation_stack --test parallel_stack`、`cargo test --release --locked --manifest-path crates/rtla-compiler/Cargo.toml --test call_depth`、rtlc の lib の unit test。
- `just browser` と `just portable-browser` を 1 回ずつ流して記録する（Chromium の `runCallDepth` を含む）。
- 共有のテスト（`tests/wasm-host/contract.ts` と browser の entry）を変えるので `just mutation`。
- `just runtime-build` / `just runtime-test` と `just call-depth-harness`（harness の `oracle-peak` の出力を変え、`stack_programs` を読むので、fmt と clippy を確かめる）。
- `just evaluate` / `just replay-evaluation` と、`just eval-loop` / `just replay-loop`。
- 文書を更新して `just support-check` を通す。
  - `crates/rtlc/SUPPORT.md`: 85 に 2 つの段と転送の規則を足し、87 の 13 種類を 17 種類にして足した 4 種類を書く。
  - `docs/safety-ir-v2.md` の 53: 引数が `Parameter` / `Capture` だけの転送は loop でたどり、hop ごとに work を 1 使い、転送の段に積むことを書く。再帰しない経路の 256 は評価の再帰の段と比べ、再帰する定義に入った経路の 128 は今と同じく転送を含めた深さと比べることも書く。

次がそろったときに採用する。

- **Red と対照**: Red から Green になる。対照は before と after の両方で通り、故障版 F1〜F7（F2a・F2b・F3a・F3b・F5a・F5b・F5c を含む）のそれぞれで、その故障に対応する対照が失敗する。
- **TLC**: 再現の hop、hop-invariant、`chain_N50.tla`、5 つの族、意味の対照が、固定 TLC と一致する。比べるのは、complete なら status・状態数・生成数、違反なら状態数・生成数と trace の全状態、エラーならエラーの種類。work とエラーの文言は、上の比較先の規則（セルの種類ごと）で決めた相手と一致する。
- **境界**: hop の N = 126 は Rust の 2 経路で、hop-invariant の N = 125 は 4 経路で、型付きの拒否になる。
- **差**:
  - hit が 0 の要求は、before と after の raw の応答・work・IR に差がない（並列と再帰の経路を含む）。v1 は差 0。
  - hit が 1 以上の要求は、`0120-oracle-after` の `published256` と一致する（例 A・B・C、3 状態の fold の入力、入れ子の fold の入力を含む。並列で hit がある要求は（判定・エラーの種類と文言・trace で比べ、work は比べない））。
  - `0120-oracle` の `published256` が before の応答と一致する（自己検査。並列で hit がある要求は（判定・エラーの種類と文言・trace で比べ、work は比べない））。
  - 予算が尽きる並列の検査は、結果の種類とエラーの種類が before の集合に入る（該当するセルが無ければ、無いことを記録する）。
  - 2 つの観測用の複製は、raw の応答と `peakTotal` が全点で一致し、`peakDepth` が予測と一致する（sweep の規則による）。
- **測定**: 上の stack・Wasm・CLI の条件を満たし、Go の各入力の行が期待値と一致する。
- **性能**: 再現する 10% 以上の後退がない。

結果を見る前に、満たさない場合の扱いを次のとおり決めておく。

- ある host の Wasm の上端（256 と比べる値）が 512 未満、またはある種類の受理の最大の大きさでの最小の stack が release で 1,024 KiB、debug で 6,144 KiB を超えた場合は、この変更を採らず、測った値を記録して保留する。上限の値は 256 のままで、この iteration では値を変えない。
- hit が 0 の要求で before と after に差がある場合、または 2 つの観測用の複製の raw の応答か `peakTotal` が 1 点でも違う場合は、意味が変わったものとして採らず、原因を直すまで評価を進めない。
- `peakDepth` が予測と違う点がある場合は、評価を流す前に条件の変更として plan に追記し、テストの大きさを直して Red を取り直す。

この変更の後も残る制約:

- 再帰する定義に入った経路は、深さ 128 で `bounded / evaluation` になる。転送の hop も含めて数えるので、TLC が受理する深い再帰は未確定のまま。before が 256 で拒否していた入力の一部は、after で `bounded / evaluation` になる（例 A）。
- 256 と比べる値が 256 以上の入力は型付きで拒否する。`Update` と `At` の spine（0119 の候補 1）と、根が `Prime` の引数は段ごとに数える。
- `UNCHANGED V8` は v2 で 256 の拒否のまま。
- debug の build を 2 MiB の thread で動かす利用者は、深さ 250 近くで abort しうる（要件は 8 MiB）。
- CLI の Wasm は v1 しか出さないので、v2 の上限は測れない。

## 危険と影響の範囲

- **影響の範囲**: v2 のすべての call の `Parameter` / `Capture` の評価がこの loop を通り、すべての `descend` が 2 つの段を読む。そのため v2 の全入力（Quint の生成物、TCommit、共有 chain、temporal、再帰する action）に効く。
  - 届く先は native の core、Wasm の core（Node・Chromium・Go の wazero）、並列の検査、`idealis` の package。v1 と CLI（v1 だけを出す）には効かない。
  - IR・wire・frontend は変えないので、保存した IR と fixture の入力はそのまま使える。
  - 観測できる違いは、before の実行が 256 の拒否に当たった要求だけに出る予定。その要求は、受理される、より深い位置で拒否される、`bounded / evaluation` になる（例 A）、並列で work が変わる（例 B）、並列で先に違反が確定する（例 C）、のどれかになりうる。
  - 並列の検査で共有の予算が尽きる要求は、今も worker が予算を取る順で結果が変わる。0120 はこの性質を変えないが、その要求は byte 単位では比べられないので、結果の種類で比べる。
- **要求の分け方の取り違え**: probe が要求ごとに初期化されないか、判定の後に記録すると、hit の数と深さの記録が別の要求や別の位置のものになる。要求の始めの初期化、判定の前の記録、深さ 300 の次に 40 を流す driver の自己検査（40 の記録が 300 にならないこと）で確かめる。
- **深さの取り違え**: 次のどれかを起こすと、深さから決まる結果が動く。合計を before の深さに保つ規則、再帰と ENABLED と snapshot の対照、F1 と F5a〜F5c で確かめる。
  - `forwarded` を引き継がない context を作る（ENABLED の subscript と step、`Snapshot`、新しく足す context）。
  - 128 の判定・葉の検査を、評価の再帰の段だけで行う。
- **意味の取り違え**: 終端の context の field を取り違えると値が変わる（`prime` を落とす、使う側の `locals` や `old` を使う、`recursive` を作り直す、根が `Prime` の実引数を素通りする）。規則 4 で今の `..context` と同じ field の分け方にし、次で確かめる: primed・束縛・`@` を終端にした対照、故障版 F3a・F3b・F4、2 つの観測用の複製の全点の照合。
- **`descend` の順の取り違え**: v1 の早い return・再帰の 128・256 の順を入れ替えると、v1 の ENABLED が拒否されるか（F7）、再帰する context で評価の再帰の段が 256 に達した位置が `bounded / evaluation` でなく protocol の拒否になる（例 A の 254 段）。今の順を明記し、この 2 つの対照で確かめる。
- **順序の取り違え**: 途中の hop の判定を省くか、参照と `consume(1)` の後に回すと、深さの拒否の代わりに work 切れや参照のエラーが出て、work も変わる。規則 3 の順序、`v2_forwarding_refuses_before_the_next_reference`、F6 で確かめる。
- **work の後退**: `consume` を鎖ごとにまとめたり、参照の後にまとめて課金したりすると、work と予算切れの位置か、エラーの順序が変わる。hop ごとの `consume(1)`、予算の全点の対照（F2a）、不正な参照と予算 7 の対照（F2b）、`evaluator-corpus` と `temporal-parity` の完全一致で確かめる。
- **stack**: 上限 256 のままでも、転送を含む入力は、今より多くの call と演算子の再帰を深さ 255 の中に入れられる（hop は N = 83 の 166 段から N = 125 の 250 段）。
  - 深さ 1 段あたりの stack は、転送の hop より call と演算子の方が大きいかもしれない。
  - `eval_reference` の frame や `EvalContext` が大きくなると、転送のない種類の stack も増える（`EvalContext` は値で渡されるので、field の追加はすべての frame に効く）。
  - loop を別の `#[inline(never)]` の関数にし、`size_of::<EvalContext>()` を記録し、stack と Wasm の上端を全種類で測り直して、0119 と同じ規則で判断する。満たさなければ採らない。
- **性能**: 転送の多い入力では関数の呼び出しが減る。一方で、葉の検査と `descend` が 2 つの段を足して比べるので、命令が増える。kernel の Criterion で後退を確かめる。
- **文書と期待値**: 期待値が変わるテストと文書は、上で宣言した範囲だけとする。
- **戻し方**: product code は `safety_expressions.rs` と `safety_lazy.rs` の 2 file なので、その commit を戻せば before の挙動に戻る。fixture と run は追記だけで、0119 の記録は変えない。

## 変更するファイル（予定）と差分の見積もり

| ファイル                                                                                           | 内容                                                                                                                                                                                          | 行数の見積もり |
| -------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------: |
| `crates/rtlc/src/safety_expressions.rs`                                                            | `forwarded` の field、`descend` と葉の検査の比べ方、ENABLED の 2 つの context、転送の loop（`#[inline(never)]` の関数）、`Argument::evaluate` の置き換え、test module の v1 の ENABLED の対照 |          約 55 |
| `crates/rtlc/src/safety_lazy.rs`                                                                   | `Snapshot` の `forwarded`                                                                                                                                                                     |           約 3 |
| `crates/rtlc/src/safety_definition_tests.rs`                                                       | 新しい unit test 19 件、遅延関数の組                                                                                                                                                          |         約 480 |
| `crates/rtlc/tests/stack_programs/mod.rs`                                                          | 4 種類の追加                                                                                                                                                                                  |          約 90 |
| `crates/rtlc/tests/evaluation_stack.rs`                                                            | `forward` の大きさ、4 種類の境界                                                                                                                                                              |          約 25 |
| `crates/rtlc/tests/parallel_stack.rs`                                                              | 転送の境界のモデル、例 B、例 C                                                                                                                                                                |         約 100 |
| `crates/rtla-compiler/tests/call_depth.rs`                                                         | hop の N、5 つの族                                                                                                                                                                            |         約 120 |
| `tests/call-depth/contract-test.ts`                                                                | 0120 の fixture、境界、拒否の後の instance                                                                                                                                                    |          約 40 |
| `tests/wasm-host/contract.ts`、`tests/portable-browser-entry.ts`、`tests/portable.browser.spec.ts` | `runCallDepth` の 125 / 126                                                                                                                                                                   |          約 10 |
| `fixtures/call-depth/`                                                                             | `records-0120.json`、`cases-0120.json`                                                                                                                                                        |         生成物 |
| `evals/experiments/call-depth/`                                                                    | `sources-0120.ts`、`expectations-0120.ts`、`expect-0120.ts`、`baseline-0120.ts`、`paths-0120.ts`、`analyze-0120.ts`、`tlc-0120.ts`、`oracle-0120.patch`                                       |         約 650 |
| `evals/experiments/call-depth/wasm-ceiling.ts`、`browser-ceiling.ts`                               | 受理の判定と `maxWork` の引数（既定は今と同じ）                                                                                                                                               |          約 30 |
| `evals/experiments/call-depth/stack-harness/src/main.rs`                                           | `oracle-peak` の出力に probe の 4 つの値                                                                                                                                                      |          約 10 |
| 文書（上の 2 ファイル）                                                                            | 2 つの段、転送の規則、17 種類                                                                                                                                                                 |           約 8 |

product code は約 58 行、テストは約 865 行、実験は約 690 行で、fixture と run は生成物。`stack.ts`、`cli-ceiling.ts`、`justfile` は変えない（種類とセルは引数と `stack_programs` から読む）。iteration と README の索引は評価の後に書く。
