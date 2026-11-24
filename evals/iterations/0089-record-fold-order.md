# 0089: 順序に依存しない集合 fold を、record・文字列を含む要素でも受理する

[#98](https://github.com/mizchi/idealis/issues/98)の実装と評価。[plan](../plans/0089-record-fold-order.md)は 6ea8530ec を基準に記録し、入れ子の fold の対照を足す追記を実装の前に記録した。push と remote merge は含めない。

rtlc の集合 fold は、要素のどこかに文字列・record があると、TLC の intern ID が IR にないことを理由に、body を評価する前に拒否していた。今回は、TLC の順序を知らなくても結果が TLC と同じになると IR から分かる fold だけを受理する。変えたのは rtlc の fold の評価と、その判定（`safety_fold_order.rs`、prepared program への登録）だけで、IR・wire・`contracts.ts`・両 frontend は変えていない。

## 受理する形

- **S**: 集合の要素が 1 個以下。要素同士の比較が起きないので、拒否の理由を問わず（model value や整数 key の幅を含む）body を評価する。
- **R0**: body が要素を読まない。毎回同じ関数を要素数だけ適用するので、値・評価エラーの文言と位置・overflow・work が順序に依らない。
- **R1**: body が累積値に `\union` で集合を足す形（累積値そのもの、`cup`、`if`、unmatched CASE の組み合わせ。足す集合と IF の条件は累積値を読まない）。値は和集合の可換性と結合性で順序に依らない。TLC が最初に報告するエラーは順序で決まるので、body の評価エラーは従来の拒否に置き換える。予算切れと `Unsupported TLA input` で始まる別の拒否はそのまま返す。

判定は fold の node ごとに、最初に文字列・record の理由を見つけた時だけ計算する。結果は prepared program が持つ（`OnceLock`）。今受理される fold はこの経路を通らない。複製された式（呼び出しの actual の中の fold）は prepared program の表にないので、その都度計算する。要素が 2 個以上で判定が受理の場合、走査は最後まで進み、整数 key の幅・Boolean などの key・model value の拒否が残っていれば、走査が最初に見つけた拒否を返す。

要素が 2 個以上で順序に依存する fold（`Append`、初期値が集合でない `\union` の評価エラー、累積値を読む足し込み、最後の要素を返す形、要素の和）は、今までと同じ文言で拒否される。

## Red から Green

- Rust: `crates/rtlc/tests/fold_order.rs` の 13 件。IR v1 の展開と v2 の `call` の両方を直接組む。before（6ea8530ec）では 9 件が順序の拒否の文言で失敗し、4 件（受理しない形の対照、今受理される fold の work、混在する理由）は通る。after では 13 件すべて通る。
- TS: `foldIndependentControls`（22 入力）を `tests/tlc-differential/regression-test.ts` で TS / Rust の両 frontend、native / Wasm、v1 / v2 で流す。before では 1,319 件中 48 件が失敗し、after では全件通る。失敗した 48 件は、宣言した 9 件と新しい受理の入力、その v1 / v2 の一致の検査だけで、ほかの既存の入力は before でも通る（[結果](../runs/0089-paths/result.json)）。

## TLC との照合

固定 TLC v1.7.4（`5a47802`、jar の sha256 は plan と同じ）で 22 入力を採取した。command・置換・1 つずつの JVM は既存のログと同じで、既存の 63 ログのバイト列は変えていない（[採取](../runs/0089-tlc/result.json)）。plan に書いた予測はすべて当たった。

- R0 は要素数だけ適用した値になった（`x = 2`、文字列 6 個で `x = 6`、`<<<<0>>>>`、2 回目の加算で overflow）。#98 の最小の例は初期状態で `x = 2` の違反、Next の対照は 0、2、4 の trace。
- R1 は和集合の値になった。LET の callback も同じ。入れ子の対照（下）は `{1, 2, 3}`。
- 要素 1 個の対照は、最後の要素 `[zeta |-> 0]` と、空列の `Head` の評価エラーになった。
- R1 の形で body がエラーになる 2 件は TLC も評価エラー。順序に依存する 4 件は `[alpha |-> 0]`、`{<<"a", 0>>, <<"b", 1>>}`、`{"a", "b"}`、`x = 3`。`Op(a, b)` の引数名が `a`・`b` を先に intern するという読みと矛盾しない。ただし `fold-dependent-if-leaf` の「逆の順なら `{"a"}`」は観測していない。これらの ID の割り当てについての読みは、D・A の判断材料としては、今回のログでも増えていない。

宣言した 9 件（要素 1 個の `fold-unsupported-{string,record}-*` 6 件、`fold-set-functions-with-records`、`fold-set-wide-single-function`、`fold-set-boolean-function-keys`）は `unsupported` を外し、既存のログの値と一致した。record の期待値は、以前は object の形で書いてあり、idealis の PortableValue（組の配列）と比べられなかったので、比べる 4 件を組の配列に直した。ほかの 10 件の `unsupported` は同じ文言で拒否される。

## 入れ子の fold

`fold-independent-union-nested`: 外側は record 2 個の集合で、body は `a \union ApaFoldSet(Inner, {}, {[v |-> 1], [v |-> b.w]})`、`Inner(xs, y) == xs \union {y.v}`。TLC は `{1, 2, 3}`。

**R1 で受理した。実装は直していない。** 外側の body は `cup(累積値, 内側の fold)` で、内側の fold は外側の累積値を読まない。外側の要素は読んでよい。内側の fold は自分の body が R1 なので、自分の判定で受理する。

- v1 では内側の fold の slot が外側の `bound` 0・1 の次（2・3）に置かれ、外側の slot と衝突しない。
- v2 では内側の callback が別の共有定義で、`parameter` の 0・1 は定義ごとの frame の中にある。外側の判定は呼ばれる定義の body を見ないので、内側の `parameter` を外側のものと数えない。

plan のとおり、入れ子の `frame` の中の parameter は、番号が同じなら読むと数える。`fold_order.rs` は、`frame` の中で自分の parameter の 0 を使う形（外側の累積値は読まない）が拒否のままであることも確かめる。これは過小に受理する側に倒れる誤りで、入れ子の fold の受理は妨げない。この数え方が、実際の frontend の出力で入れ子の fold を拒否する例は、今回の入力では見つからなかった。

## 今受理される入力は変わらない

- 完全応答 corpus（Quint 6・TCommit・共有 chain の 11 入力、v1 / v2、native / Wasm / cold native、各 4 iteration）の 264 比較が、work を含めて before と一致した（[summary](../runs/0089-corpus-summary/result.json)）。
- 旧版 156 モデル・1,560 要求の時間論理の比較は、before / after の native / Wasm で完全に一致した（[reduced](../runs/0089-temporal-parity/reduced.json)）。
- `crates/rtla/src`、`crates/rtla-compiler/src`、`crates/rtlc/src/ir.rs`、`packages/idealis/src`、`crates/idealis-core/src` は基準と byte 単位で同じ（[sources](../runs/0089-audit/sources.json)）。変わった rtlc の source は 4 ファイルと、新しい 1 ファイル（`safety_fold_order.rs`）。
- 整数の集合 fold の work は v1 で 35、v2 で 71 と固定し、before でも同じ値で通る。
- 観測の 14 の集合 × 6 の body の 84 入力は、before では全部が順序の拒否だった。after は 63 入力で順序の拒否を返さず、残る 21 入力（`Append`、累積値を読む足し込み、初期値が集合でない場合の 3 種 × 要素 2 個以上の 7 集合）は before と同じ文言で拒否される。63 のうち 7 入力は、要素 1 個の集合に初期値が集合でない `\union` を足す形で、body が `Expected a finite set` の評価エラーになる（TLC も評価エラーになる形）。v1 と v2 の結果は 84 入力すべてで同じ（[表](../runs/0089-paths/grid-after.tsv)）。
- `just browser` は 7 件通った。

## 性能

- 14 本の kernel Criterion（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）の変化は −3.25% から +8.05% で、10% 以上の後退はなかった。いちばん大きい `search/counter-product/8` は、after の 2 回の差が 12.8% あって揺れており、同条件の ABBA での再確認は +0.22% だった（[kernel](../runs/0089-kernel-summary/result.json)、[再確認](../runs/0089-abba-1-summary/result.json)）。
- rtla-compiler の `quint_generated` 42 本は −10.38% から +4.34%、`compatibility` 27 本は −5.83% から +1.74%。+4.34% の `quint-check/NestedFold/v1` の再確認は −0.90%、同じ fold の v2 は +7.76%（before 同士の差が 5.5%）で、10% 以上の後退はない（[結果](../runs/0089-compiler-abba/result.json)）。この比較は frontend の source を変えていないので、compile の費用は変わらず、check の変化は評価の側の揺れの範囲に収まっている。
- 新しい経路の費用: 64 要素の集合を R1 の body で fold する Next を持つモデルの `check` を after で測った（[結果](../runs/0089-new-path/result.json)）。record の集合は同じ大きさの整数の集合の 4.8〜5.2 倍（v1）、3.8〜4.8 倍（v2）。この中には、record の値を作る費用と、走査が文字列の key を見る費用が含まれる。状態数を 1 から 64 にすると、record の `check` は v1 で 62.9 倍、v2 で 56.1 倍になった（整数は 58.0 倍と 44.5 倍）。どちらも 64 倍を超えないので、判定を状態ごとにやり直す費用は出ていない。判定を 1 回しか計算しないことは、`safety_prepared.rs` のテストでも確かめた。

測定中の負荷（load average）は 2〜10 の間で、ほかのプロセスの影響を受けている。ABBA の before 同士・after 同士の差は kernel で 0.0〜12.8% だった。

## 採用の判断

採用する。

- Red から Green は満たした。
- 宣言した 9 件は固定 TLC のログの値と一致し、ほかの 10 件の `unsupported` と新しい拒否の対照は今と同じ文言で拒否される。
- 許す差の外で、結果・work・IR に差はない。
- 再現する 10% 以上の後退はない。

`just mutation` は流していない。TS のライブラリの source（`packages/idealis/src`）を変えていないため。ほかの確認（`just check`、`wasm-test`、`safety-test`、`rtlc-test`、`compiler-test`、`quint-test`、`portable-build` / `portable-test`、`evaluate` / `replay-evaluation`、`support-check`）はすべて通った（[コマンド](../runs/0089-audit/commands.tsv)）。`support-sync` は流していない。

評価中に起きたことを記録する。corpus の最初の実行は scratch の名前で走らせ、ledger に別の名前の行ができたので、その行は残さず、同じ入力を `0089-corpus` の名前で流し直した。`just evaluate` が追記した ledger の行も残していない。

## 残る範囲

- 要素が 2 個以上で順序に依存する body（`Append`、最後の要素を返す形、要素の和、累積値を読む足し込み）の fold は拒否のまま。手元のモデルで実際に問題になったら、D（比較の結果が intern ID の割り当てに依らず決まる集合）を、それでも足りなければ A（frontend が TLC の intern の順の表を IR に出す）を検討する。D と A は、今回の受理と競合せず、後から足せる。
- R1 の形は plan のとおり狭い。`\cap`、累積値から引く `\`、整数の和（途中の和が 32 bit を超えるかが順序で変わる）、最大・最小、Boolean の `\/`・`/\`（短絡で評価する要素が順序で変わる）は入れていない。`acc \union E1 \union E2` のように `cup` が 2 段になる形も、今回は受理しない。
- 走査が最初の文字列・record を見つけた後に続けて進む分の work は、受理の経路でだけ消費する。その途中で予算が尽きた場合は、拒否ではなく予算切れを返す。
