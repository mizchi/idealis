# 0074: 連言・選言と ELSE IF の連鎖を 1 段の入れ子として数える

[#82](https://github.com/mizchi/idealis/issues/82)で[#63](https://github.com/mizchi/idealis/issues/63)の候補を扱う。[事前plan](../plans/0074-nesting-chains.md)を 0c425fdaa を基準に記録し、実装の段で変えた条件を plan に追記した。push と remote merge は含めない。

Quint が出す長い線形の連鎖（括弧つき左結合の二項の `/\`・`\/` と、`IF ... ELSE IF ...`）を、safety IR v2 の Rust / TS frontend で 1 段の入れ子として数える。今受理される入力の IR は v1・v2 とも byte 単位で変えていない。IR・wire・checker（rtlc）と、各層の上限の値も変えていない。

## 採用した変更

- 連鎖は、同じ種類の `And`（または `Or`）が直接の項になっているもの、または `IF` の `ELSE` が直接 `IF` であるものとする。Label・LET・演算子の呼び出しは連鎖を切る。時間論理の連言・選言は対象外。
- 連鎖は明示的な stack の loop でたどり、段ごとに再帰しない。各項と各枝は、連鎖の先頭で `nested()` を 1 回通した context で lower する。
- 式と membership の位置では、連言・選言は左の spine だけをたどり、今と同じ左結合の二項を作る。IF 連鎖も今と同じ入れ子の `if` を作る。右結合の入れ子と THEN 側の入れ子は今と同じく段ごとに数える。
- action・SPECIFICATION の parts・ENABLED の位置では、左右どちらの入れ子もたどる。branch の組み立て順と `limited` の検査位置は、入れ子のときと同じにした。
- 部分式を lower する順序（最初に報告するエラー）は変えていない。v1 は数え方も IR も変えていない。
- Rust（`typed_expr.rs`、`typed_lower.rs`）と TS（`tla-safety.ts`）に同じ規則を実装した。`crates/rtlc/src`、`safety-program.ts`、`contracts.ts` の 26 ファイルは、before・after と評価の終了時で sha256 が一致した。

## テストと故障版

`crates/rtla-compiler/tests/nesting_chains.rs`（11 件）と `tests/nesting-chains/contract-test.ts`（250 件）を実装の前に置いた。before では Rust が 8 件、TS が 91 件失敗し、どれも `expression or operator nesting exceeds 40` の拒否が原因だった。after では全件通った。期待値は、変更前のコードで検査した `fixtures/nesting-chains/baseline.json` から導いた。連鎖の綴りの期待値は同じ意味の綴りの変更前の結果と等しいとし、長さの閾値は別にリテラルで検査する。

実装に故障を植えて、テストが検出するかを確かめた。Rust で 3 種、TS で 5 種を植え、すべて検出された。

- 外側の frame の束縛を数え落とす。
- ELSE IF を逆順に組み立てる。
- 外側の層ごとに第 1 項を lower し直す。この故障版は指数的に遅くなり、終わらなかった。
- 左の連鎖を外側から畳む。
- ELSE IF で選ばれた枝を落とす。
- SPECIFICATION の入れ子の連言を逆順にたどる。
- Or 連鎖の最後の `limited` 検査を飛ばす。

最初は、frame の束縛を数え落とす故障版が両 suite で生き残った。1 つの木に 2 つの束縛を持つ入力を shape corpus に加え、束縛を持つ Rust の AST テストも加えると、同じ故障を検出するようになった。

## 独立レビューと修正

独立レビューは、評価の後の実装と記録に次の不具合を示した。

1. TS frontend の action の選言（`actionChain` の `Or`）が、項を足すたびに蓄積した branch の配列を複製していた。n 項の平たい選言で費用が O(n^2) になり、枝の上限を超える入力でも最後の `limited` まで払っていた。変更前の `flatMap` は線形だった。
2. 完了した内側の `Or` の `limited` を外した故障版が、既存の 250 件では検出されなかった。上の「Or 連鎖の最後の `limited` 検査を飛ばす」故障版は検出されたと記録しているが、初回の記録からは外した検査の位置を特定できない。
3. `And` の途中の `limited` を外した故障版も、250 件では検出されなかった。

1 は、`Or` の蓄積を末尾への追加に変えて直した。同じ種類の蓄積を両 frontend のほかの連鎖の経路でも調べ、項ごとの複製が変更前より増えていたのはこの 1 か所だけだった。Rust 側は変えていない。2 と 3 は、枝の上限での診断の順序を検査する group `limit`（25 件）を加えて直した。Next・ENABLED・SPECIFICATION の parts に、4,097 項の選言や 13 個の `(x = 0 \/ x = 1)` の連言を置き、その後に未定義の演算子を続ける。期待値は 0c425fdaa を build し直して得た値で、既存の 240 件の期待値は byte 単位で変わっていない。費用には、Next の 100,000 項の選言を子 process で 5 秒・1 GiB の上限つきで検査するテストを加えた。

| 100,000 項の選言（TS） |                   時間 |                   最大 RSS |
| ---------------------- | ---------------------: | -------------------------: |
| 変更前（0c425fdaa）    |                0.90 秒 |                     749 MB |
| 修正前の実装           | 4.7 秒で上限により停止 | 1.09 GB（停止時の sample） |
| 修正後の実装           |          0.67〜0.70 秒 |                  約 760 MB |

時間と RSS は、size テストと同じ子 process の計測を 1 回（変更前、修正前）または 3 回（修正後）流した値。修正前の実装では、この size テストが 1 GiB の上限で失敗した。修正の担当の手元の計測では、上限なしで約 10 秒・1.39 GB だったが、その出力は記録に残していない。2 と 3 のテストは修正前の実装でも通り、それぞれの検査を外した故障版で失敗した。以前に植えた 8 種の故障も、修正後のコードで改めてすべて検出された。テストは Rust 12 件・TS 282 件になった。Rust のテストは debug build での時間を抑えるため、group `limit` のうち 4 件だけを v2 で検査する。25 件すべてと v1 は、TS の contract が Rust frontend の native / Wasm を含む 4 経路で検査する。

修正の後、TS frontend に関わる評価をやり直した。

- 4 経路の比較: 244 入力の 1,952 セルが、最初の after と結果も IR も一致した。group `limit` の 200 セルは期待値と一致した。
- 既存の入力の IR: 27,906 セルが before と一致した。
- `just browser`: 7 件すべて通った。
- Rust の native core と Wasm は、最初の評価の after と sha256 が同じだった。そのため、Rust core だけを使う完全応答・時間論理の照合・Criterion は流し直していない。

[修正の記録](../runs/0074-r2-fix/)と[再評価](../runs/0074-r2-audit/)を参照。

レビューは、記録の不備も示した。

- iteration の表が書式検査を通らなかった。
- Jev の同期を済んだと書いていた。
- corpus の環境記録に、ホストの一時ディレクトリが残っていた。

表は整え、Jev の記述は実際の状態に直し、一時ディレクトリは `<tmp>` に置き換えた。

## TLC との比較と閾値

公開用に作った合成モデル LongConjunction（中置 `and` の 44 項）と LongIfChain（54 枝）を、固定版 Quint 0.33.1 と Apalache 0.63.0 で生成し直した。生成物の sha256 は plan の値と一致した。Quint の CLI の版は、生成に使った同じ CLI で後から記録した（[quint-version.txt](../runs/0074-quint-regen/quint-version.txt)）。

| 入力                   | 固定 TLC 1.7.4           | after v2（Rust の native / Wasm）                             | v1（before・after） |
| ---------------------- | ------------------------ | ------------------------------------------------------------- | ------------------- |
| LongConjunction safety | complete 6 状態 / 7 生成 | complete 6 / 7、work 20                                       | frontend 40 で拒否  |
| LongConjunction broken | 違反、trace 5            | 違反、trace 5、work 15                                        | 同上                |
| LongIfChain safety     | complete 56 / 57         | compile を通り、検査で `Safety expression nesting exceeds 48` | 同上                |
| LongIfChain broken     | 違反、trace 54           | 同上                                                          | 同上                |

TLC は、cases.json の 240 件と Quint の 4 件の計 244 入力を `-Xmx256m -Xss8m -workers 1` で 1 つずつ検査した。ケースごとに scratch と metadir を分けた。after v2 の Rust native は、そのうち 192 件で状態・生成数・違反 trace の長さ・エラーの種類が TLC と一致した。before は 119 件だった。残りの入力のうち、各層の上限で拒否されたものは次のとおり。

- checker 48 で拒否: 25 件
- frontend 40 で拒否: 14 件
- parser 64 で拒否: 12 件

TLC と食い違ったのは `let-between-and-10` の 1 件だけで、before でも同じ結果だった。この入力は入れ子の LET で同名の定義を作る。SANY は不正な TLA+ として拒否するが、idealis は受理する。この差は変更前からあり、本変更とは関係しない。ほかに TLC と受理・拒否が食い違ったのは、上の各層の上限による拒否と、TS frontend が `\div`・`%`・Quint の複数 EXTENDS を持たないことによる拒否だけだった。

v2 で最上位の invariant に置いたときの受理の範囲は、plan の予測どおりだった。

- 括弧つきの連言・選言は 48 項まで受理し、49 項から checker 48 で拒否する。平たい連言と同じ。
- IF 連鎖と OTHER つきの CASE は 47 枝まで受理し、48 枝から checker 48 で拒否する。
- 62 段は、Rust の parser が `TLA parsing: Expression nesting exceeds 64` で拒否する。
- v1 の括弧つき連言は、before と同じく 24 項まで受理する。25 項から checker 48 で拒否し、39 項以降は frontend 40 で拒否する。

[TLC](../runs/0074-tlc/)、[Quint の再生成](../runs/0074-quint-regen/)、[経路と差分](../runs/0074-paths/)を参照。

## 4 経路と同じ意味の綴り

244 入力を、IR v1 / v2 と 4 経路（TS → Wasm、TS → native、Rust の Wasm、Rust の native）で before と after の両方に流した。244 × 2 × 4 = 1,952 セル。

- 同じ frontend の Wasm と native の結果は、before・after とも完全に一致した。
- TS と Rust の IR は、正規化すると一致した。
- after の結果は、cases.json の 240 入力の 1,920 セルすべてで、実装時に記録した期待値と一致した。Quint の 4 入力（32 セル）はこの集計に含まれず、上の TLC との表で照合した。
- v1 は before と差が 0 セルで、結果も IR の sha256 も一致した。
- v2 で before と変わったのは 93 ケースの 334 セルで、すべて before が `expression or operator nesting exceeds 40` で拒否していたセルだった。それ以外の変化はない。after での内訳は次のとおり。
  - 受理: complete 118 セル、違反 60 セル
  - 検査時のエラー: 非 Boolean 44 セル、0 除算 22 セル
  - 検査時の checker 48 の拒否: 50 セル
  - compile 時の拒否: 未定義の operator 12 セル（報告するエラーの順は before と同じ）、TS の `\div` 非対応 28 セル

同じ意味の綴り（括弧つきの連言・選言には平たい綴り、IF 連鎖には OTHER つきの CASE、action の位置には平たい綴り）とも比べた。

- 両方を compile できる 370 組では、v2 の IR が byte 単位で一致した。
- 連鎖の after の結果は、同じ意味の綴りの before の結果（work を含む）と 436 組中 420 組で一致した。
- 残る 16 組は、Rust の parser が連鎖の側を拒否する組で、比べる IR がない。62 段の括弧つきの連言・選言・IF と、44 段の右結合の Next・ENABLED・SPECIFICATION が当たる。TS の経路では、同じ意味の綴りと一致した。
- 右結合の action の位置は Rust では end-to-end に通らない。そのため Rust 側の根拠は、AST を直接組んだテストだけになる。

Chromium は `just browser` で検査した。最初の実行は、固定した Playwright 1.65.1 が使う Chromium を入れていなかったため、7 件とも起動に失敗した。その Chromium を入れた再実行では 7 件すべて通った。2 回とも [browser](../runs/0074-browser/) に残した。

## 既存の入力と完全応答

- 既存の入力の IR: 重複を除いた 4,651 入力を、before・after の TS・Rust Wasm・Rust native で、v1・v2 ともに compile した。
  - 入力は `corpora()` の 1,268 件、analysis corpus の 3,168 件、analysis-boundary の 209 件、inventory の構文 context 6 件。`corpora()` は parser corpus、tlc-differential と operator-sharing の cohort、fixtures、nesting と shape を含む。
  - 27,906 セルで、program の sha256 または拒否の文言がすべて一致した。
  - before で `exceeds 40` に拒否されていた既存の入力 26 件は、after でも同じ拒否だった。plan で許した差（受理に変わる、または後の層で拒否される）は 0 件だった。
  - TS と Rust の正規化 IR が異なるセルは 89 あり、before と after で同じ集合だった。
- 完全応答: `evaluator-corpus`（Quint6・TCommit・共有 chain、11 入力・8 経路・各 4 iteration）の 264 比較が、work を含めて一致した。比べたのは状態・生成数・深さ・status・reason・violation・work・program。
- 時間論理: `just temporal-parity` の旧版 156 モデル・1,560 要求が、before・after の native・Wasm で一致した。
- `just evaluate` と `just replay-evaluation` は通った。

[既存の入力の IR](../runs/0074-ir-corpora/)、[完全応答](../runs/0074-corpus-summary/)、[時間論理](../runs/0074-temporal-parity/)を参照。

## stack

plan の二分探索の harness を before と after に当て、compile の段に要る最小の stack を測った（debug の v2。単位は KiB、分解能は 4 KiB）。

| n                   |   3 |   8 |  20 |    38 |    39 |   40 |   61 |
| ------------------- | --: | --: | --: | ----: | ----: | ---: | ---: |
| 括弧つき連言 before | 163 | 291 | 627 | 1,107 | 1,123 | 拒否 | 拒否 |
| 括弧つき連言 after  | 147 | 147 | 147 |   179 |   179 |  179 |  259 |
| IF 連鎖 before      | 195 | 323 | 643 | 1,123 |  拒否 | 拒否 | 拒否 |
| IF 連鎖 after       | 147 | 147 | 147 |   179 |   179 |  195 |  275 |

- 61 段の連鎖は、debug の 2 MiB の thread で compile を通る。
- 39 段以下で、after の最小 stack が before を超えるセルはない。
- after にも、段数に比例する成分が残る。38 段から 61 段で 1 段あたり、括弧つき連言は約 3.5 KiB、IF 連鎖は約 4.2 KiB で、before は約 27 KiB だった。連鎖をたどる loop 以外に、`crates/rtla-compiler/src/levels.rs` の level 解析が今も段ごとに再帰する。
- v1 は before と after で同じ値だった。
- serialize・decode・check・eval の段は、38 段で before と after が同じ値だった。

[stack](../runs/0074-stack/)を参照。

## Criterion

0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で比べた。対象は rtla-compiler の 5 本の bench（`quint_generated`・`frontend`・`frontend_semantics`・`compatibility`・`recursive_actions`）と、新しい `nesting_chains`。合わせて 138 関数で、変化は after の平均 / before の平均 − 1 とした。

- ABBA-1 は、同じホストで別の検査が走っていて 1 分 load の中央値が 11 だった。変更していない parse・check・prepared の関数を含む 35 関数が 10% 以上遅くなった。どの関数も after の 2 回のうち 1 回だけが外れていた。
- ABBA-2 は load の中央値 1.9 で、10% 以上の後退は `quint-parse_SetFold` の +10.9% の 1 件だけだった。parse は変更の対象外で、`quint_generated` 全体が +4.9% に偏った run だった。前半の 2 本（`nesting_chains` と `quint_generated`）は load が高い中で走った。
- ABBA-3 は `quint_generated` と `nesting_chains` を負荷の低い時間に流し直した。`quint-parse_SetFold` は +3.9% で、10% 以上の後退は 0 件だった。
- 3 回すべてで 10% 以上遅くなった関数はなく、再現する後退はない。
- `nesting_chains`（括弧つき連言と IF 連鎖の n = 3, 8, 38 の compile）は、3 回とも after が速かった。ABBA-3 では -5% から -26%。
- rtlc は変えていないので、plan では kernel の Criterion を流さないとした。ノイズの目安として 1 回流すと、14 関数で -0.7% から +2.9% だった。

[ABBA-2](../runs/0074-abba-2/)、[ABBA-3](../runs/0074-abba-3/)、[kernel](../runs/0074-kernel-summary/)、[実行順とlog](../runs/0074-audit/)を参照。ABBA-1 は、判断に使わない失敗として [0074-abba-1](../runs/0074-abba-1/) に残した。評価の途中の誤りも 0074-audit に残した。bench の filter を誤って 1 件も走らなかった kernel の 1 回目と、driver の誤りで落ちた初回の経路の実行がそれに当たる。初回の経路の実行の log は再試行の前に消してしまい、exit code と原因（driver が cwd 相対で wasm を読んだこと）だけが commands.tsv と注記に残る。

## 検査

- rtla-compiler の cargo test・fmt・clippy（`-D warnings`）
- `just check`、`just compiler-test`、`just safety-test`、`just analysis-test`
- `just wasm-test`、`just quint-test`、`just portable-test`
- `just browser`、`just evaluate`、`just replay-evaluation`、`just support-plan`
- Jev の checklist の同期（`just support-sync`）と、その後の `just support-check`。9 scope すべて complete（rtla 8/8、rtla-compiler 25/25、rtlc 21/21、idealis-core 6/6、rusmt 4/4、ralloy 4/4、idealis-cli 4/4、idealis-connect 6/6、roracle 6/6）。#85 の rebase の後は、Jev の checklist は Git の対象外の任意の助言になったので、同期した checklist は記録に含めない。

`just check` は pnpm 経由で `pnpm-lock.yaml` を書き換えるので、その lockfile は戻してから記録した。

記録した run では、実行したホストの絶対パスとユーザー名を、記録する時点で placeholder（`<scratch>`、`<after>`、`<before>`、`<clone>`、`<tools>`、`<mise>`、`<home>`、`<user>`、`<host>`、`<tmp>`）に置き換えた。結果の値、計数、sha256 は変えていない。

## 記録の縮小

rebase の後、`evals/runs/0074-*` を iteration が引用する summary だけに縮めた（`evals/experiments/nesting-chains/reduce-runs.ts`）。元の記録は 12,902 ファイル・約 75 MB で、そのうち 12,591 ファイル・74.1 MB を消し、311 ファイル・約 1.5 MB を残した。値・計数・sha256 は書き換えていない。

- 消したもの: Criterion の sample の directory（ABBA と kernel）、ケースごとの出力（p0 corpus の応答、TLC の入力と log）、before / after の IR と経路の dump、時間論理の応答の全文、`*.log`、Git から再現できる tracked diff、fixtures と同じ Quint の生成物と Apalache server の出力、browser の失敗時の context。
- 残したもの: 表、`criterion-summary.txt`、各 bench の `runs.json`、`analysis.txt`・`differences.tsv` などの集計、TLC の `results.json` と `verdicts.json`、manifest、`commands.tsv`、終了コード、sha256 の一覧、driver。時間論理は、応答ごとの sha256 を `summary.json` に要約した。
- 各 run の `reduced.json` に、消したファイルの数・バイト数・一覧の digest（sha256sum の行を path 順に並べた sha256）と、256 KiB 以上のファイルの sha256 を残した。元の記録は公開していない。`reduced.json` の digest は、元の記録を持つ者が消した内容を照合するためのもので、公開した記録だけでは消した内容を検証できない。

## 残る制約

連鎖をたどる処理は段ごとに再帰しなくなったが、式の位置の連鎖では IR の深さが今も長さに比例する。そのため、次の入力は今も拒否される。

- 54 枝の LongIfChain は検査時に checker 48 で止まる。
- 49 項以上の連言と 48 枝以上の CASE は checker 48 で止まる。plan の観測では、さらに長いと frontend の出力検査 128 で止まる。
- plan の観測では、連鎖を呼び出しの中に重ねた式は動的評価 128 で止まる。

128 の 2 つの上限は、この評価では測っていない。

手元のプロジェクトの Quint 生成物にも、54 枝の IF 連鎖と同じ形がある。これらは #82 の次の iteration 0075 で扱う。0075 では v2 の IR に、連言・選言・CASE を 1 つの node で表す形を加え、各項と各枝を node の直下で評価する。そうすれば、IR の深さも checker の深さも長さに比例しなくなる。

その他の制約は次のとおり。

- Rust の parser は、最も外側まで括弧で囲んだ 62 段以上の連鎖を拒否し、TS は parse する。この差は変更前からある。右結合の連言は、plan の観測では Rust の parser が 30 段を受理し 32 段で拒否する。この評価の測定点は 30・39・40・44 段だった。
- 右結合の action の位置は、Rust では AST を直接組んだテストだけが根拠になる。
