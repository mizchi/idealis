# 0074: 連言・選言と ELSE IF の連鎖を 1 段の入れ子として数える

[#82](https://github.com/mizchi/idealis/issues/82)で[#63](https://github.com/mizchi/idealis/issues/63)の候補として扱う。基準は 0c425fdaa（origin/main）。[0044](../iterations/0044-quint-generated.md) の Quint 生成物の対応範囲を広げる。Quint が出す長い線形の連鎖（左結合の二項の `/\`・`\/` と、`IF ... ELSE IF ...`）を、safety IR v2 の Rust / TS frontend で 1 段の入れ子として数える。今受理される入力の IR は、v1・v2 とも byte 単位で変えない。IR・wire・checker（rtlc）と、上限の値（frontend 40、checker の式 48、動的評価 128、JSON decoder と TS decoder の 128、frontend の出力検査 128、parser 64）は変えない。push と remote merge は含めない。

## 観測

公開用に作った合成モデルを、`fixtures/quint-generated` と同じ固定版（Quint 0.33.1、Apalache 0.63.0、jar の sha256 f2d43f2e279a657be50b2ecec3a013ba2915266008a2562b38412c3269347dc9）で TLA+ にした。中置の `and` / `or` は括弧つきの左結合の二項に、`and { ... }` は平たい列に、`if ... else if` は入れ子の `IF` に、`match` は `CASE` になる。

```tla
safety == ((((x /= 10 /\ x /= 11) /\ x /= 12) /\ x /= 13) /\ ...) /\ x /= 53
label == IF x = 0 THEN 100 ELSE IF x = 1 THEN 101 ELSE ... ELSE IF x = 53 THEN 153 ELSE 0
```

44 項の連言（LongConjunction）と 54 枝の IF 連鎖（LongIfChain）の生成物の sha256 は、それぞれ da139c2f1461064a3197165c315c7aa136221184e67c6473e33ed2d4fa9b3954 と 335653adbc2afd1efacd0628465f31a054b68c66f771e3dea0b2dacfcb2dcc98。どちらも現在の native の `check-tla` と `compile-tla` で、v1・v2 とも `Unsupported TLA input: expression or operator nesting exceeds 40` で拒否される。`EXTENDS Integers` に置き換えた同じ式は、TS frontend でも v1・v2 とも同じ文言で拒否される。固定 TLC は既定の stack で、連言の safety を 6 状態・7 生成で complete、broken を 5 状態で違反とする。IF 連鎖では safety が 56 状態・57 生成で complete、broken が 54 状態で違反になる。61 段の連鎖も TLC は既定の stack で検査できる。手元のモデルでも同じ形で拒否される。

手で書き換えると、次の層で止まる。平たい 44 項の連言は v1・v2 とも受理される（v2 の work 20）。54 枝を CASE にすると、v1・v2 とも検査時に `Safety expression nesting exceeds 48` になる。連鎖を定義の呼び出しの中に置くと、v2 の動的評価で `Safety evaluation nesting exceeds 128` になる。

最上位の invariant に置いたとき、今受理される最大の長さは次のとおり。

| 連鎖                 |  v1 |  v2 | 次の長さでの拒否                                                                  |
| -------------------- | --: | --: | --------------------------------------------------------------------------------- |
| 括弧つき左結合の連言 |  24 |  39 | v1 は checker 48（v1 は各二項に TRUE を足すので IR が約 2 倍）、v2 は frontend 40 |
| 平たい連言           |  47 |  48 | checker 48                                                                        |
| IF / ELSE IF         |  37 |  38 | frontend 40                                                                       |
| CASE                 |  47 |  47 | checker 48                                                                        |
| 括弧つき・IF の共通  |  61 |  61 | parser（`TLA parsing: Expression nesting exceeds 64`）                            |

深さは、連鎖の 1 段ごとに次の場所で増える。

- frontend: AST の 1 段ごとに `Context::nested` を通る（`crates/rtla-compiler/src/lib.rs:105-117`、v2 の式は `typed_expr.rs:51`、action は `typed_lower.rs:153`、SPECIFICATION の parts は `typed_lower.rs:753-762`、TS は `packages/idealis/src/tla-safety.ts:257-260, 1903`）。
- IR: v2 の連言・選言は第 1 項から左へ畳んだ二項になる（`typed_expr.rs:256-273`、TS `tla-safety.ts:2085-2102`）。IF と CASE は入れ子の `if` になる（`typed_expr.rs:230-255, 459-483`、TS `tla-safety.ts:2068-2083, 2267-2287`）。
- checker: 静的検証の 48（v1 は `crates/rtlc/src/safety_expressions.rs:642-645`、v2 は `safety_definitions.rs:79-81, 223-225, 259-261, 646-648`）、v2 の動的評価の 128（`safety_expressions.rs:112-127, 806-815`）。
- 境界: 要求 JSON の decoder の再帰 128（`crates/idealis-core/src/lib.rs:587-595`、serde_json の既定）、TS decoder の 128（`packages/idealis/src/safety-program.ts:166-168`）、frontend の出力検査の 128（`crates/rtla-compiler/src/typed.rs:276-300`）、parser の 64（`crates/rtla/src/lib.rs:19`、`parser.rs:312-313, 386-387`）。

frontend は連鎖の段ごとに数えるが、出力する IR は綴りによらない。今の v2 は、両 frontend とも次の組に同じ IR を出す（n = 3, 6, 20 で確認）。

- 式の位置: 括弧つき左結合の連言と平たい連言。IF 連鎖と OTHER つきの CASE（membership の位置でも同じ）。
- action の位置: 括弧つき・平たい・箇条書き・右結合の連言。IF 連鎖と OTHER つきの CASE。

そのため、括弧つきの 44 項の連言は、平たい 44 項の連言と同じ IR を作れば検査を通る。action の位置の連鎖は step と branch に lower され、IR は平たい。それでも 44 項の括弧つき連言や 54 枝の IF を Next や ENABLED に置くと、frontend の 40 で拒否される。

上限の目的は、native と Wasm の stack を守ること。上限の定数だけを外した複製（評価専用）で、1 段あたりに要る最小の stack を二分探索した（v2、括弧つき連言と IF 連鎖、各段を別 thread で実行）。

| 処理           | native release |  native debug | Wasm（stack 1 MiB で通る最大の段数）                   |
| -------------- | -------------: | ------------: | ------------------------------------------------------ |
| parse          |        約 5 KB | 約 15 / 29 KB | `compile-tla`（parse と compile）で 641〜642 段        |
| compile        | 約 3.5〜3.8 KB |      約 27 KB | 同上                                                   |
| JSON の decode | 約 1.2〜1.3 KB |     約 4.8 KB | `check-safety`（decode・検証・評価）で 1,918〜2,048 段 |
| 検証と評価     |      約 0.9 KB |    約 26.5 KB | 同上                                                   |

Rust のテストが既定で使う 2 MiB の thread（debug）では、64 段で parse（IF）が 1,891 KiB、compile が 1,827 KiB を使う。上限を上げる案を支えるのは、この条件で 64 段程度までなので、上限の値は変えない。debug build で 128 の動的評価の拒否より前に stack overflow になる件は、別の [#83](https://github.com/mizchi/idealis/issues/83) で扱う。

## 仮説

連鎖をたどる処理を段ごとの再帰から loop に変え、連鎖全体を 1 段の入れ子として数えれば、frontend の stack は連鎖の長さに比例しなくなる。出力する IR は、今受理される同じ意味の綴り（平たい連言、OTHER つきの CASE）の IR と同じにできるので、評価順序・短絡・エラー・work と checker の上限は変わらない。

- 連鎖は、同じ種類の `And`（または `Or`）が直接の項になっているもの、または `IF` の `ELSE` が直接 `IF` であるものとする。Label・LET・演算子の呼び出しは越えない。
- 連鎖は明示的な stack の loop でたどり、段ごとに再帰しない。各項と各枝は、連鎖で `nested()` を 1 回通した context で lower する。
- 式と membership の位置では、連言・選言は左の spine だけをたどり、今と同じ左結合の二項を作る。IF 連鎖も今と同じ入れ子の if を作る。右結合の入れ子は今と同じく段ごとに数える。
- action、SPECIFICATION の parts、ENABLED の位置では、左右どちらの入れ子もたどる。branch の組み立ての順と `limited` の検査の位置は、入れ子のときと同じにする。各枝は先頭から lower し、組み立ては内側から行う。
- 部分式を lower する順序（最初に報告するエラー）は変えない。
- v1 は変えない。数え方も今のまま。

両 frontend で同じ規則を実装する。v2 の最上位の invariant では、括弧つき連言が平たい連言と同じ 48 項まで、IF 連鎖が CASE と同じ 47 枝まで、action の位置の連鎖が parser の上限まで受理される見込み。時間論理の式（`TemporalFormula`）の連鎖は対象外とする。

## 固定条件と対照

- before は 0c425fdaa の source / binary。after は before にこの変更だけを加えたもの。それぞれの sha256 を記録する。`crates/rtlc/src`、`packages/idealis/src/safety-program.ts`、`packages/idealis/src/contracts.ts` が変わらないことを sha256 で確かめる。
- テストを先に置き、before で意図した理由（`expression or operator nesting exceeds 40`）で失敗すること（Red）を両 frontend で記録する。Rust は `crates/rtla-compiler/tests/nesting_chains.rs`、TS は `tests/nesting-chains/` に置く。固定入力と期待値は `fixtures/nesting-chains/`、生成と計測は `evals/experiments/nesting-chains/` に置く。既存テストの期待値は変えない。既存テストが失敗したら、実装を直すか、評価の前に条件の変更として plan に追記する。
- 合成の入力（公開用に作ったもので、特定の対象の名前を含まない）:
  - Quint の LongConjunction（中置 `and` の 44 項）と LongIfChain（54 枝）。Quint 0.33.1 と Apalache 0.63.0（上の jar）で、既存の `quint-generate` と同じ手順で生成し、Rust frontend で検査する。生成物の sha256 が上の値と一致することを確かめる。
  - TS と共通の TLA+（`EXTENDS Integers`）: 上の 2 つ、平たい 44 項の連言、54 枝の CASE と、長さ n ∈ {2, 3, 4, 24, 25, 38, 39, 40, 44, 47, 48, 49, 54, 61, 62} の括弧つきの連言・選言・IF 連鎖。乱択は使わない。
  - 意味の対照（n = 44、位置 p ∈ {1, 22, 23, 44}）: p の項が偽になる違反、p の偽が後の 0 除算を短絡する違反、p の 0 除算が後の偽より先に起きるエラー、p の非 Boolean によるエラー、選言の真による短絡、IF 連鎖の第 p 枝の選択と未選択の枝の 0 除算、第 p 条件の 0 除算と非 Boolean。
  - 位置の対照: Next に置いた 44 項の括弧つき連言と 54 枝の IF、ENABLED の 44 項の連言、45 個の初期条件と `[][Next]_x` を括弧つきで連ねた SPECIFICATION、membership の位置の 44 枝の IF。
  - 変えない境界の対照: `~` と `Id(...)` の入れ子の 39 / 40 / 41、`/\` と `\/` が交互に入れ子になる式、式の位置の右結合の連言、`\cup` の連鎖、IF の THEN 側の入れ子、時間論理の連言、既存の `deep_drop` と手書き IR の 48 / 49、動的評価の 127 / 128 / 129。before と after で同じ結果を要求する。
- IR の期待値: 新しく受理される入力の IR は、同じ意味の綴りを before で compile した IR と byte 単位で一致することを要求する。対応は、括弧つきの連言・選言には平たい綴り、IF 連鎖には OTHER つきの CASE、action の位置には平たい綴り。v1 の出力と拒否は before と同じであることを要求する。
- 結果の期待値: status・状態数・生成数・反例は固定 TLC から取る。TLC は v1.7.4（revision 5a47802b、jar の sha256 936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88）を `-Xmx256m -Xss8m -workers 1` で 1 つずつ実行し、scratch と metadir をケースごとに分ける。work は、同じ意味の綴りを before で検査した値と一致することを要求する。同じ意味の綴りを before が拒否する入力（54 枝の IF 連鎖など）は、after でも同じ層の同じ文言で拒否されることを要求する。54 枝の IF 連鎖は compile を通り、検査で 54 枝の CASE と同じ `Safety expression nesting exceeds 48` になる。
- 既存の入力の IR: Rust frontend の corpus（`evals/rtla-parser/corpus.ts` の入力、tlc-differential と operator-sharing の cohort、`fixtures/tlc-case`、`fixtures/quint-generated`）と、analysis の inventory・corpus の全入力を、before / after の両 frontend で compile する。v1・v2 の IR と拒否の文言が完全に一致することを要求する。違いは、before で `expression or operator nesting exceeds 40` だった連鎖の入力が受理されるか、後の層で拒否される場合だけ許す。その入力は一覧にして保存する。
- 完全応答: 既存の `evaluator-corpus`（Quint6・TCommit・共有 chain、8 経路・各 4 iteration）を before / after で流し、compiled program・結果・work の完全一致を要求する。旧版 156 モデル・1,560 要求（`just temporal-parity`）も native / Wasm で照合する。
- 経路: Rust frontend の native / Wasm、TS frontend から native / Wasm の 4 経路で、両 frontend の IR の一致と結果の一致を要求する。Chromium は `just browser` を 1 回流して記録する。Rust の parser が拒否する長さ（最上位で 62 段以上）は TS（tla-ts）が parse するため、frontend の一致には数えず、両方の結果を記録する。この差は変更前からあり、冗長な括弧を 70 段重ねると TS は受理し、Rust は parse で拒否する。揃える作業はこの iteration に含めない。
- stack: 上の二分探索の harness を after に当て、61 段の連鎖の compile が debug の 2 MiB の thread で通ること、39 段以下での compile の最小 stack が before を超えないことを確かめる。
- Criterion: 0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で、rtla-compiler の `quint_generated`・`frontend`・`frontend_semantics`・`compatibility`・`recursive_actions` の compile の費用を比べる。新しい bench `nesting_chains`（括弧つき連言と IF 連鎖の n = 3, 8, 38 の compile）を before と after の両方に同じ source で置いて比べる。10% 以上の後退は同条件の ABBA で再確認し、再現する後退を残したまま採用しない。rtlc は変えないので、kernel の Criterion は流さない。
- build・test・benchmark は同時に流さない。上限を外した複製は観測にだけ使い、時間とメモリの上限をつけ、状態数が有界な入力に限る。

## 記録と完了条件

run は追記のみ。source / binary / 入力・driver・実行順・予算・host・全 sample を保存し、失敗も残す。記録する時点で、実行したホストの絶対パスとユーザー名を placeholder に置き換える。

終了前に `just check`、`just wasm-test`、`just safety-test`、`just compiler-test`、`just quint-test`、`just portable-build` / `just portable-test`、rtla-compiler の fmt / clippy、`just evaluate` / `just replay-evaluation` を実行する。`docs/safety-ir-v2.md` と `docs/rust-frontend.md` の入れ子の数え方の記述と、`crates/rtla-compiler/SUPPORT.md` を更新し、`just support-plan` / `just support-sync` で Jev の比較を確認する。API key がない場合の skip は、検査済みと区別して記録する。

Red から Green、TLC との一致、既存の入力の IR の一致、完全応答の一致、再現する後退がないことが揃ったときに採用する。採否と残る制約を iteration と README に戻す。

この変更の後も、式の位置の連鎖は IR の深さが長さに比例するので、次の入力は拒否される。54 枝の LongIfChain は rtlc の検証 48 で止まる。49 項以上の連言（さらに長いと frontend の出力検査 128）、48 枝以上の CASE、連鎖を呼び出しの中に重ねた式（動的評価 128）も拒否される。これらは同じ #82 の次の iteration 0075 で扱う。0075 では v2 を拡張し、連言・選言・CASE を 1 つの node で表して、各項と各枝を node の直下で評価する。平衡二分木は、最後の項を深くして今受理される program を 128 で拒否することを観測したので使わない。

## 実装の段で変えた条件

- `fixtures/tlc-operator-sharing/analysis-inventory.json` の `extracted` を更新した。変更した `typed_expr.rs`・`typed_lower.rs`・`tla-safety.ts` の sha256 と、lowering の証跡（文字列と関数名）を機械で再抽出した値に置き換えた。既存の inventory のテストは、この 2 つが現在のソースと完全一致することを要求する。分類と位置の記録は変えていない。
- v2 の期待値は、変更前のコードで検査した `fixtures/nesting-chains/baseline.json` から導出した。連鎖の綴りの期待値は同じ意味の綴りの変更前の結果と等しいとし、v1 と連鎖でない入れ子は変更前の結果そのものとした。長さの閾値（平たい連言と選言 48、CASE と IF 連鎖 47、62 段は Rust だけ parser で拒否）は、別にリテラルで検査する。
- Rust のテストは 64 MiB の thread で動かす。parser の上限に近い入力は、変更前から debug の 2 MiB の thread で parse だけで stack overflow になるため。
- TS frontend は `\div` と `%` を持たないので、0 除算の対照は Rust frontend だけで結果を検査する。TS では、連鎖と平たい綴りがともに `expression Apply \div` で拒否されることを検査する。両 frontend で通す対照は、非 Boolean の項と条件を使う。
- v1 の CASE（OTHER つき）の閾値は、このテストの包み（`label == CASE ...`、`Inv == label >= 0`）で 44 枝受理・47 枝拒否だった。上の表の 47 とは包みが違う。v1 は変えないので、観測値を期待値とした。
- 式位置の右結合の連言は引き続き段ごとに数える。Rust の parser は右結合の連言を 30 段で受理し 32 段で拒否するため、Rust の右結合と action 位置の左右の入れ子は、AST を直接組んだテストで確かめる。
- Quint の生成物（LongConjunction、LongIfChain）は、設計の段の固定版（Quint 0.33.1、Apalache 0.63.0）の出力をそのまま `fixtures/nesting-chains/quint/` へ置き、sha256 を plan の値と照合した。生成スクリプトは追加しない。
- レビューの後に TS frontend の action の選言（`actionChain` の `Or`）の蓄積を、項ごとに配列を複製する形から末尾への追加へ変えた。flat な選言の n 項で複製が O(n^2) になり、上限を超える入力でも最後の `limited` まで払っていたため。IR・branch の順序・報告するエラーは変えない。Rust 側（`typed_lower.rs`）は項ごとに蓄積を移動して `limited` を通すだけで、変更前と同じ費用なので変えていない。
- 枝の上限での診断の順序を `fixtures/nesting-chains/` に足した（group `limit`。Next、ENABLED、SPECIFICATION の parts に置いた 4,097 項の選言と、13 個の `(x = 0 \/ x = 1)` の連言のあとの未定義の演算子）。期待値は 0c425fdaa を fresh に build して `baseline.json` を再生成した値で、既存の 240 件は変更前と byte 単位で一致し、新しい件は実装の出力から取っていない。完了した内側の `Or` の `limited` と、`And` の途中の `limited` をそれぞれ外した植え込みが、既存の 250 件では検出されなかったことへの対応。
- 費用の上限は `fixtures/nesting-chains/sizes.json` に置いた。Next の 100,000 項の選言（`\/` を行頭に並べた形）は、変更前の 0c425fdaa で TS 0.90 秒、Wasm 1.42 秒で枝の上限により拒否される（記録した計測）。修正前の実装は TS で 4.7 秒の時点でメモリの上限（1 GiB）に当たって止まる。テストは先例（operator-sharing-limits-test）と同じ子 process で 5 秒・1 GiB の上限を課し、結果は変更前の値と一致させる。項を中置の `\/` で連ねる形は、変更前から parser の費用が項数に対して超線形で、50,000 項で TS が約 8 秒かかり、修正の差が埋もれるため使わない。メモリの上限は、変更前の TS の実測が最大 749 MB だったため、先例の 256 MiB ではなく 1 GiB とした。
- Rust のテストの group `limit` は debug build での時間のため、v2 の 4 件（`limit-next-or-flat-4097`、`limit-next-and-chain-13`、`limit-next-or-inner-then-missing`、`limit-next-and-chain-then-missing`）だけを検査する。25 件すべてを v2 で検査すると debug で約 59 秒かかった。group `limit` の全件と v1 は、TS 側の contract が Rust frontend の native / Wasm を含む全 path で検査する。ENABLED と SPECIFICATION の placement は、診断の順序が分かれる 4 形（`or-inner-then-missing`、`or-missing-then-inner`、`and-chain-then-missing`、`and-right-then-missing`）に限った。
