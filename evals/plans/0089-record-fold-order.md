# 0089: 順序に依存しない集合 fold を、record・文字列を含む要素でも受理する

[#98](https://github.com/mizchi/idealis/issues/98)の計画。基準は 6ea8530ec（origin/main）。`ApaFoldSet` の集合が文字列・record をどこかに含むと、rtlc は TLC の intern ID がないことを理由に拒否する（`crates/rtlc/src/safety_values.rs` の `check_fold_order`、841〜903 行）。この iteration では、TLC の順序を知らなくても結果が TLC と同じになると証明できる fold だけを受理へ変える。変えるのは rtlc の fold の評価だけで、IR・wire・`contracts.ts`・両 frontend は変えない。今受理される入力の結果・work・IR も変えない。push と remote merge は含めない。

## 観測

### 今の拒否

6ea8530ec の Rust の API（parse → `compile_with_options` の v1 / v2 → `SafetyProgram::check`）で測った。

- `fixtures/tlc-apalache-folds` の順序の対照のうち、文字列・record の 14 件（`fold-unsupported-{string,record}-{direct,tuple,set}` の 6 件と、`fold-set-strings`・`fold-set-strings-reversed`・`fold-set-records`・`fold-set-record-fields`・`fold-set-nested-strings`・`fold-set-sets-of-strings`・`fold-set-functions-with-strings`・`fold-set-functions-with-records` の 8 件）は、v1・v2 とも `check` が protocol エラーで拒否する（文字列 8 件、record 6 件）。
- 同じ 14 の集合で、body だけを順序に依存しない形に替えた 70 入力（`a + 1`、`a \union {b}`、`{<<b>>} \cup a`、初期値が集合でない `a \union {b}`、累積値を読む `a \union {<<b, Cardinality(a)>>}` の 5 種）も、v1・v2 とも同じ文言で拒否した。拒否は `fold_members` の中で body を評価する前に起きるので、body の形を見ていない。
- issue の最小の例（`Step(acc, r) == acc + 1` を 2 つの record に fold）も、v1・v2 とも record の文言で拒否した。

v2 の IR では、fold の body は `call`（arguments が `bound` の累積値の slot と要素の slot）で、callback の共有定義の body は `parameter` 0 と 1 を使う。v1 では body が呼び出し位置に展開され、`bound` の 2 つの slot を直接読む。LET 内の callback（Quint の生成物の形）も、Rust frontend の v2 で同じ形になった。

### リポジトリの中の件数

固定入力・テスト・実験の source（`evals/runs` を除く）で `ApaFoldSet` を使う箇所をすべて見た。文字列・record の拒否に当たるのは上の 14 件だけで、すべて body が `Append(a, b)` である。これらは順序を観測するために置いた対照なので、body は順序に依存する。ほかの集合 fold（Quint の SetFold と NestedFold、`sharing-binder-fold-set`、cached-expansions と expansions の fixture、compiler のテスト）は整数の集合で、今も受理される。

`fixtures/tlc-apalache-folds/expected.json` で `unsupported` を持つのは 19 件。上の 14 件に、整数 key の幅の 4 件と Boolean key の 1 件が加わる。このうち要素が 1 個の集合は 9 件ある（文字列・record の `fold-unsupported-*` 6 件、`fold-set-functions-with-records`、`fold-set-wide-single-function`、`fold-set-boolean-function-keys`）。9 件とも、固定 TLC のログに値がある。

手元のモデルの集合 fold には、累積値に `\union` で集合を足す形、要素を読まない形、最後に見た要素を返す形（順序に依存する）が混ざっている。Quint の sum type は `tag` が文字列の record になるので、variant の集合の fold はどれも今は拒否される。件数は数えていない。

### TLC の順序

固定 TLC v1.7.4（`5a47802`）のログと source から、次のことが分かる。

- 文字列の比較は `UniqueString.compareTo` の `this.tok - other.tok` で、intern ID の差だけを見る（[UniqueString.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/util/UniqueString.java#L222-L227)）。record は field の数を比べ、次に intern ID の順に並べた field 名と値を交互に比べる（[RecordValue.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/RecordValue.java#L75-L101)）。
- 識別子の token は、parser が syntax tree の node を作るときに intern する（[SyntaxTreeNode.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tla2sany/parser/SyntaxTreeNode.java#L168-L188)）。record の field 名はこの識別子の token をそのまま使う（[Generator.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tla2sany/semantic/Generator.java#L3819-L3820)）。
- 文字列の literal は、parse の時点では引用符を含む token のまま intern される。引用符を外した中身は、意味解析の段で `StringNode(treeNode, true)` が intern する（[Generator.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tla2sany/semantic/Generator.java#L2840-L2841)、[StringNode.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tla2sany/semantic/StringNode.java#L30-L39)）。ログでは、TLC は root module を最初に parse し、その後に EXTENDS した module を parse する。意味解析は依存の順で、root module が最後になる。
- 評価中に作る文字列も、その時点で新しい ID を得る。`\o`・`Tail`・`SubSeq` は新しい `StringValue` を作り（[Sequences.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/module/Sequences.java#L152)）、`UniqueString.concat` も intern する（[UniqueString.java](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/util/UniqueString.java#L187-L190)）。rtlc もこの 3 つを評価する。

既存のログはこの読みと矛盾しない。`fold-set-strings`（source の順は `"b", "a", "A", "aa", "B", ""`）の TLC の順は `a, b, A, aa, B, ""`、`fold-set-strings-reversed`（`"", "B", "aa", "A", "a", "b"`）は `a, b, "", B, aa, A`。どちらも、先に parse される `Op(a, b)` の引数名が `a`・`b` を最初に intern し、残りは literal が現れた順になる。`fold-set-record-fields` の `zeta` が `alpha` より先なのも、field 名の token が現れた順である。ただし、既存のログはどれも「parse の時点の intern」と「意味解析の時点の intern」を区別できない。設定ファイルの parse が spec より先に intern するか、TLC の起動時に事前登録される文字列が何かも、ログからは分からない。

## 候補

TLC との食い違いの危険が小さい順に並べる。数は、上の 14 件（リポジトリで今拒否される文字列・record の fold）と、14 の集合 × 6 種の body（`Append` と上の 5 種）の 84 入力について、各候補だけを入れたときに受理されるもの。

| 候補 | 受理する条件                                        | 変える場所                                  | 大きさ | TLC と食い違う危険                                                      | 14 件 | 84 入力 |
| ---- | --------------------------------------------------- | ------------------------------------------- | ------ | ----------------------------------------------------------------------- | ----: | ------: |
| S    | 集合の要素が 1 個以下                               | rtlc の fold 評価                           | 数行   | なし（要素の間の比較が起きない）                                        |     7 |      42 |
| R0   | body が要素を読まない                               | rtlc の fold 評価と IR の静的な判定         | 小     | なし（毎回同じ関数を n 回適用する。エラーと work も順序に依らない）     |     0 |      14 |
| R1   | body が累積値に `\union` で集合を足す形             | R0 と同じ                                   | 小〜中 | 値はなし。どのエラーが先かは順序で変わるので、body のエラーは拒否に戻す |     0 |      28 |
| D    | 比較の結果が intern ID の割り当てに依らず決まる集合 | rtlc の比較                                 | 中     | 中（TLC の比較を ID の不明な部分を残したまま写す必要がある）            |     8 |      48 |
| A    | frontend が TLC の intern の順の表を IR に出す      | IR・wire・`contracts.ts`・両 frontend・rtlc | 大     | 高（下の理由）                                                          |    14 |      84 |

S・R0・R1 を合わせると、14 件のうち 7 件、84 入力のうち 63 入力を受理する。残る 21 入力は、要素が 2 個以上で、body が `Append`、初期値が集合でない `\union`、累積値を読む `\union` のもの。

1. **S（採る）。** 要素が 1 個以下なら、TLC も要素同士を比べないので、順序の問題が起きない。文字列・record の拒否だけでなく、整数 key の幅、Boolean key、model value の拒否にも同じ理由が当てはまるので、要素が 1 個以下なら種類を問わず受理する。既存の 9 件（上の「リポジトリの中の件数」）は、固定 TLC のログの値をそのまま期待値にできる。
2. **R0（採る）。** body の中に要素の参照が 1 つもなければ、fold は初期値に同じ関数を要素数だけ適用したものになる。値・評価エラーの文言と位置・overflow・work が、どの順でも同じになる。issue の最小の例はこの形である。
3. **R1（採る）。** body が累積値に `\union` で集合を足し、足す集合と IF の条件が累積値を読まないなら、結果は初期値と各要素の集合の和になり、順序に依らない。値の一致は和集合の可換性と結合性による。一方で、足す集合の評価がエラーになる要素が複数あると、TLC が最初に報告するエラーは TLC の順序で決まる。初期値が集合でないときのエラーの文言も、最初に足す集合を含みうる。そこで R1 の経路では、body の評価がエラーになったら、今と同じ順序の拒否を返す。work の予算切れと、`Unsupported TLA input` で始まる別の拒否は、そのまま返す。
4. **D（採らない。次の候補として残す）。** 例えば field の集合が同じで、値が 1 つの field でだけ違う 2 つの record は、ID の割り当てがどうであっても TLC の比較の結果が決まる。比較の結果が決まる組だけで整列が終われば、その順は TLC の順と一致する（整列の手順が観測する比較の結果が、どの ID の割り当てでも同じなので、出力も同じになる）。14 件では `fold-set-records` が加わって 8 件になる。ただし、Quint の sum type の集合は `tag` の文字列で比べるので、variant が 2 種類以上あると決まらない。手元のモデルで多い variant の集合にはほとんど効かず、TLC の record・関数・集合の比較を「決まらない」を含めて写す手間が要る。R0・R1 で残る fold が実際に問題になったら扱う。
5. **A（採らない）。** 上の「TLC の順序」から、表を正しく作るには少なくとも次が要る。
   - 固定した TLC が起動時に事前登録する文字列の一覧と、その登録が parse より先かどうか。
   - 設定ファイルの parse が intern するか、spec より先か。
   - root module の識別子の token の順と、EXTENDS・INSTANCE した module の parse の順。これには標準 module（jar の中の Integers・Sequences・FiniteSets・TLC など）と、Apalache の module（fold の fixture は 0.56.1、Quint の生成物は 0.63.0 で、版で中身が違う）の token の列が要る。Quint の生成物は `EXTENDS Variants` を持ち、`tag`・`value` が root に現れなければ Variants の parse の位置で ID が決まる。
   - 文字列の literal の中身を intern する意味解析の順（module の依存の順と、module の中の走査の順）。
   - 評価中に作る文字列（`\o`・`Tail`・`SubSeq`）の ID。TLC の評価の順に依存するので、表では決まらず、拒否が残る。

   どれも既存のログでは確かめられず、仮説ごとに新しい TLC の採取が要る。さらに、IR の版、wire、`contracts.ts`、TS frontend と Rust frontend の両方の生成、rtlc の比較を変える。14 件の対照をすべて受理できる唯一の候補だが、順序に依存する body が手元のモデルで実際に問題になるまでは採らない。

6. **組み合わせ。** S・R0・R1 を先に入れる。その後、順序に依存する fold が実際に残ったら D を、それでも足りなければ A を検討する。D と A は S・R0・R1 と競合せず、後から足せる。

R1 の形を広げる候補（`\cap`、累積値から引く `\`、要素から計算した整数の和、最大・最小、Boolean の `\/`・`/\`）は今回は入れない。整数の和は、途中の和が 32 bit を超えるかどうかが順序で変わり、TLC の overflow のエラーが順序に依存する。正と負の部分和の上限と下限を評価時に確かめれば受理できるが、別の判定になる。`\/`・`/\` は短絡で評価する要素が順序で変わる。どれも R1 と同じく、手元のモデルで必要になったときに別の iteration で扱う。

## 仮説

S・R0・R1 を入れると、宣言した 9 件と下の新しい入力のうち受理の行が、固定 TLC と同じ判定・値・エラーになり、拒否の行は今と同じ文言で拒否される。観測の 84 入力では 63 入力が受理に変わり、残る 21 入力は今と同じ文言で拒否される（84 入力は TLC で採取しないので、受理か拒否かと、native と Wasm・v1 と v2 の一致だけを確かめる）。今受理される入力は、IR・結果・work が before と完全に一致する。

予測（結果を見る前に固定する）。

- 既存の 9 件の要素 1 個の fixture は、ログの値どおりの invariant 違反になる。ほかの 10 件の `unsupported` は今と同じ文言で拒否される。
- issue の最小の例は、初期状態で `x = 2` の invariant 違反になる。
- 新しい対照の予測値は下の「固定条件と対照」の表に書く。TLC の採取の結果が予測と違えば TLC の値を期待値にし、外れを iteration に記録する。

## 受理する形

判定は IR だけを見る。Expression の木をたどり、次の形だけを受理する。形に合わなければ拒否のまま（過小に受理する方向で誤る）。

- body の取り出し: v1 では fold の `body` そのものを見る。累積値は `bound` の `accumulator`、要素は `bound` の `accumulator + 1`。v2 では `body` が `call` で、`arguments` がちょうど `[bound accumulator, bound accumulator + 1]` のときだけ、その定義の body を見る。累積値は `parameter` 0、要素は `parameter` 1。`captures` に累積値か要素の slot が現れたら受理しない。ほかの形（frame、別の引数の並び）は受理しない。
- 参照の数え方: 対象の木のどこかに要素の参照（v1 は `bound accumulator + 1`、v2 は `parameter` 1）があれば「要素を読む」とする。入れ子の `frame` や `call` の引数・capture の中も数える。入れ子の frame が自分の parameter として同じ番号を使う場合も読むと数え、受理しない側に倒す。累積値も同じ。
- R0: 要素を読まない。
- R1: 次の U の形。
  - U は、累積値の参照そのもの、`cup`（左右の一方が累積値の参照で、もう一方 E が累積値を読まない）、`if`（条件 C が累積値を読まず、両方の枝が U）、`unmatched-case` のどれか。
  - CASE は frontend が `if` の連鎖と `unmatched-case` へ下ろすので、同じ規則で扱う。
- 判定は、`check_fold_order` が文字列か record の理由で拒否する場合にだけ使う。今受理される fold はこの経路を通らないので、結果と work が変わらない。判定の結果は prepared program ごとに fold の node 単位で 1 回だけ計算し、状態ごとに木をたどり直さない。

fold の評価は次の順にする。

1. 今と同じ走査（`tick` も同じ）で集合の要素を調べる。今受理されるものは今と同じに処理する。
2. 走査が拒否を見つけた場合、要素が 1 個以下なら、拒否の理由を問わず、その要素で body を評価する（S）。
3. 要素が 2 個以上で、拒否の理由が文字列か record だけなら、R0 か R1 の判定を見る。走査は、文字列・record を見つけた後も、整数 key の幅・Boolean などの key・model value の拒否が残っていないかを最後まで調べる。それらが 1 つでもあれば今の拒否を返す。
4. R0 なら、rtlc の集合の順で body を評価し、エラーもそのまま返す。R1 なら同じ順で評価し、body のエラーは、予算切れと `Unsupported TLA input` の拒否を除いて、走査が見つけた今の拒否の文言に置き換える。
5. どちらでもなければ今の拒否を返す。

R0 と R1 の work は rtlc の集合の順で決まる。TLC には work がないので TLC と比べず、native と Wasm、両 frontend、v1 と v2 で一致することだけを求める。

## 固定条件と対照

- before は 6ea8530ec の source / binary、after は before にこの変更だけを加えたもの。それぞれの sha256 を記録する。`crates/rtla/src`、`crates/rtla-compiler/src`、`crates/rtlc/src/ir.rs`、`packages/idealis/src`、`crates/idealis-core/src` が before と after と評価の終了時で変わらないことを sha256 で確かめる。変わるのは `crates/rtlc/src` の fold の評価と判定だけとする。
- テストを先に置き、before で意図した理由（順序の拒否の文言）で失敗すること（Red）を記録する。Rust は `crates/rtlc/tests/fold_order.rs` に置き、IR を直接組んで、v1 の展開と v2 の call の両方の形を検査する。受理しない形（captures の参照、入れ子の frame の同じ番号、累積値を読む E、累積値でない葉）、走査が見つける受理しない理由（文字列・record と model value の混在など）、R1 のエラーの置き換えと予算切れの素通し、今受理される fold の work が変わらないことも置く。TS の 4 経路は `evals/experiments/tlc-differential/corpus.ts` に新しい一覧を足し、`tests/tlc-differential/regression-test.ts` から流す。
- 期待値が変わる既存のテストを、実装の前にここで宣言する。`fixtures/tlc-apalache-folds/expected.json` の要素 1 個の 9 件から `unsupported` を外し、ログの値を期待値にする。ほかの 10 件の `unsupported` は変えない。`crates/rtlc/src/safety_values.rs` の unit test と、`unsupported` を見る TS のテストのうち、この 9 件に当たるものも変わる。それ以外の既存のテストの期待値は変えない。分類と違うテストが失敗したら、実装を直すか、評価の前に条件の変更として plan に追記する。
- 期待値は固定 TLC から取る。TLC は既存のログと同じ v1.7.4（revision 5a47802b、jar の sha256 936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88）、Apalache 0.56.1 の `Apalache.tla`（sha256 a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac）で、`fixtures/tlc-apalache-folds/README.md` と同じ command・置換・1 つずつの JVM で採取する。既存の 63 ログのバイト列は変えない。値は `Prop == x # x` の初期状態の違反から取る（`fold-independent-issue` と `fold-independent-next` を除く）。
- 新しい入力（すべて `goldenFrontendParity`。定義は `foldValue` と同じ module の形で、`Op` 以外の定義は書いたとおり）:

| 名前                              | 定義と式                                                                                                                                         | 受理の理由                             | 予測する TLC の結果                  |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------- | ------------------------------------ |
| `fold-independent-count-records`  | `Op(a, b) == a + 1`、`ApaFoldSet(Op, 0, {[zeta \|-> 0], [alpha \|-> 0]})`                                                                        | R0                                     | `x = 2`                              |
| `fold-independent-count-strings`  | 同じ body、初期値 0、`{"b", "a", "A", "aa", "B", ""}`                                                                                            | R0                                     | `x = 6`                              |
| `fold-independent-constant`       | `Op(a, b) == <<a>>`、初期値 0、`{"b", "a"}`                                                                                                      | R0（累積値には依存し、要素は読まない） | `x = <<<<0>>>>`                      |
| `fold-independent-overflow`       | `Op(a, b) == a + 1`、初期値 2147483646、`{[zeta \|-> 0], [alpha \|-> 0]}`                                                                        | R0                                     | 2 回目の加算で overflow の評価エラー |
| `fold-independent-issue`          | #98 の最小の例そのもの（`Prop == x # 2`）                                                                                                        | R0                                     | 初期状態で `x = 2` の違反            |
| `fold-independent-next`           | `Init == x = 0`、`Next == IF x < 4 THEN x' = ApaFoldSet(Op, x, {[a \|-> 1], [b \|-> 2]}) ELSE UNCHANGED x`、`Safe == x # 4`、`Op(a, b) == a + 1` | R0                                     | 0、2、4 の trace で違反              |
| `fold-independent-union-records`  | `Op(a, b) == a \union {b}`、初期値 `{}`、`{[zeta \|-> 0], [alpha \|-> 0]}`                                                                       | R1                                     | `{[zeta \|-> 0], [alpha \|-> 0]}`    |
| `fold-independent-union-strings`  | 同じ body、初期値 `{}`、`{"b", "a", "A", "aa", "B", ""}`                                                                                         | R1                                     | 同じ 6 要素の集合                    |
| `fold-independent-union-left`     | `Op(a, b) == {<<b>>} \cup a`、初期値 `{}`、`{<<"b">>, <<"a">>}`                                                                                  | R1                                     | `{<<<<"a">>>>, <<<<"b">>>>}`         |
| `fold-independent-union-project`  | `Op(a, b) == a \union {b.a}`、初期値 `{}`、`{[a \|-> 2], [a \|-> 1, b \|-> 0], [a \|-> 1]}`                                                      | R1                                     | `{1, 2}`                             |
| `fold-independent-union-if`       | `Op(a, b) == IF b.a > 1 THEN a \union {b} ELSE a`、初期値 `{}`、同じ集合                                                                         | R1                                     | `{[a \|-> 2]}`                       |
| `fold-independent-union-variants` | `Op(a, b) == a \union {b.value}`、初期値 `{}`、`{[tag \|-> "B", value \|-> 1], [tag \|-> "A", value \|-> 2]}`                                    | R1（sum type の形）                    | `{1, 2}`                             |
| `fold-independent-union-let`      | `LET Op(a, b) == a \union {b} IN ApaFoldSet(Op, {}, {"b", "a"})`（top-level の `Op` は置かない）                                                 | R1（LET の callback）                  | `{"a", "b"}`                         |
| `fold-singleton-last`             | `Op(a, b) == b`、初期値 0、`{[zeta \|-> 0]}`                                                                                                     | S                                      | `[zeta \|-> 0]`                      |
| `fold-singleton-error`            | `Op(a, b) == Head(<<>>)`、初期値 0、`{"only"}`                                                                                                   | S                                      | 空列の Head の評価エラー             |
| `fold-independent-union-error`    | `Op(a, b) == a \union {b.a}`、初期値 `{}`、`{[a \|-> 1], [b \|-> 2]}`                                                                            | R1 の形だがエラー → 拒否               | 存在しない field の評価エラー        |
| `fold-independent-union-nonset`   | `Op(a, b) == a \union {b}`、初期値 0、`{"b", "a"}`                                                                                               | R1 の形だがエラー → 拒否               | 集合でない値の和の評価エラー         |
| `fold-dependent-last`             | `Op(a, b) == b`、初期値 0、`{[zeta \|-> 0], [alpha \|-> 0]}`                                                                                     | 拒否（順序に依存）                     | `[alpha \|-> 0]`                     |
| `fold-dependent-reads-acc`        | `Op(a, b) == a \union {<<b, Cardinality(a)>>}`、初期値 `{}`、`{"b", "a"}`                                                                        | 拒否（E が累積値を読む）               | `{<<"a", 0>>, <<"b", 1>>}`           |
| `fold-dependent-if-leaf`          | `Op(a, b) == IF b = "a" THEN {b} ELSE a \union {b}`、初期値 `{}`、`{"b", "a"}`                                                                   | 拒否（葉が累積値でない）               | `{"a", "b"}`（逆の順なら `{"a"}`）   |
| `fold-dependent-sum`              | `Op(a, b) == a + b.v`、初期値 0、`{[v \|-> 1], [v \|-> 2]}`                                                                                      | 拒否（今回の範囲外）                   | `x = 3`                              |

拒否の行は fixture の `unsupported` に今の文言を書き、TLC の値かエラーを記録する。`fold-dependent-reads-acc` と `fold-dependent-if-leaf` の予測は、`Op(a, b)` の引数名が `a`・`b` を先に intern するという上の読みに基づく。外れたら、その読みの誤りとして iteration に記録する（A の判断の材料にもなる）。

- 経路: Rust frontend の native / Wasm、TS frontend から native / Wasm の 4 経路と、v1 / v2 で before と after を流す。同じ frontend の native と Wasm の結果の一致、両 frontend の IR の正規化した一致を要求する。frontend を変えないので、両 frontend の IR は before と after で byte 単位で同じになることも要求する。Chromium は `just browser` を 1 回流して記録する。
- 既存の入力の結果: 完全応答の `evaluator-corpus`（Quint 6・TCommit・共有 chain、8 経路・各 4 iteration）、旧版 156 モデル・1,560 要求の `just temporal-parity`、`tests/tlc-differential` の既存の全入力を before / after で流し、上で宣言した 9 件を除いて、program・結果・work の完全一致を要求する。
- 許す差: 上で宣言した 9 件の受理と、新しい入力だけ。それ以外の差は 0 件を要求する。
- Criterion: 0063 と同じ条件（20 samples、warmup 0.5 秒、measurement 1 秒、before / after / after / before）で、14 関数の kernel と rtla-compiler の `quint_generated`・`compatibility` を流す。10% 以上の後退は同条件の ABBA で再確認し、再現する後退を残したまま採用しない。新しい経路の費用は、Next で 64 個の record の集合を R1 の body で fold するモデルと、同じ大きさの整数の集合を fold するモデルの `check` を after で測り、比を記録する（閾値は置かない）。判定を状態ごとにやり直していないことは、状態数を 1 と 64 に変えた同じモデルの比で確かめる。
- build・test・benchmark は同時に流さない。

## 記録と完了条件

run は追記のみ。`evals/runs/0089-*` に、source / binary / 入力・driver・実行順・予算・host・終了コードを保存し、失敗も残す。README の規則に従い、commit するのは iteration が引用する summary だけにし、大きな出力は消した数・バイト数・digest を `reduced.json` に残す。記録する時点で、実行したホストの絶対パスとユーザー名を placeholder に置き換える。

予定する run は `0089-tlc`（新しいログの採取）、`0089-paths`、`0089-corpus`、`0089-temporal-parity`、`0089-kernel` / `0089-kernel-summary`、`0089-abba-N`、`0089-browser`、`0089-evaluate`、`0089-audit`。

終了前に `just check`、`just wasm-test`、`just safety-test`、`just rtlc-test`、`just compiler-test`、`just quint-test`、`just portable-build` / `just portable-test`、`just evaluate` / `just replay-evaluation` を実行する。`crates/rtla-compiler/README.md` の集合 fold の段落、`fixtures/tlc-apalache-folds/README.md`（入力の件数、対照の表、S・R0・R1 の説明）、`docs/rust-frontend.md` と `docs/rust-safety.md` の fold の順序の記述、`crates/rtlc/SUPPORT.md` と `crates/rtla-compiler/SUPPORT.md` の集合 fold の項目を更新し、`just support-check` を通す。TS のライブラリの source は変えないので `just mutation` は対象外とし、その理由を記録する。

次がそろったときに採用する。

- Red から Green。新しい受理の入力は before で順序の拒否、after で TLC と同じ判定・値・エラー。
- 宣言した 9 件が固定 TLC のログの値と一致し、ほかの 10 件の `unsupported` と、新しい拒否の対照が今と同じ文言で拒否される。
- 許す差の外で、before と after の結果・work・IR に差がない。
- 再現する 10% 以上の後退がない。

採用しても、要素が 2 個以上で順序に依存する body（`Append` など）の fold は拒否のまま残る。その扱いは D と A の候補として iteration に残し、#98 には S・R0・R1 で受理する範囲と残る範囲を書く。

## 追記（2026-11-24）: 入れ子の fold を R1 の対照に加える

実装に入る前の依頼で、R1 の対照を 1 件増やす。結果を見る前の予測と受理の条件を、ここで固定する。plan の記録を整えるのは採取と実装の後になったが、予測値 `{1, 2, 3}` は `crates/rtlc/tests/fold_order.rs` の `nested` に、TLC の採取の前に書いた。

### 新しい入力

`fold-independent-union-nested`（`goldenFrontendParity`）。外側の fold は record 2 個の集合で、body は累積値に内側の fold の結果を `\union` で足す。内側の fold も record 2 個の集合で、body は `xs \union {y.v}`（R1）。内側の集合は外側の要素を読む。

```tla
Inner(xs, y) == xs \union {y.v}
Op(a, b) == a \union ApaFoldSet(Inner, {}, {[v |-> 1], [v |-> b.w]})
Init == x = (ApaFoldSet(Op, {}, {[w |-> 2], [w |-> 3]}))
```

- 予測する TLC の結果は `x = {1, 2, 3}`（`b.w` が 2 のとき `{1, 2}`、3 のとき `{1, 3}`）。TLC の値が違えば TLC の値を期待値にし、外れを iteration に記録する。
- 受理の理由は R1 の入れ子。外側の body は `cup`（左が累積値、右が内側の fold）で、右は外側の累積値を読まない。要素を読むのは R1 で許す。内側の fold は、自分の body が R1 なので、自分の判定で受理する。
- v1 では内側の fold の slot が外側の `bound` の 0・1 の次（2・3）に置かれ、外側の slot と衝突しない。v2 では内側の callback は別の共有定義で、`parameter` の 0・1 は定義ごとの frame の中にある。外側の判定は呼ばれる定義の body を見ないので、内側の定義の parameter 番号を数えない。この数え方で拒否されるなら、実装を直して iteration に書く。健全でないと分かったら拒否のままにして、理由を iteration に書く。
- 回帰は `foldIndependentControls`（TS の 4 経路と v1 / v2）と `fold_order.rs`（v1 と v2 の IR を直接組む）に置く。

### 実装中に決めたこと

- 追加の入力は、plan の表の 20 件とこの 1 件の計 22 件を `foldIndependentControls` にまとめる。TLC のログは 22 件を同じ command・置換・1 つずつの JVM で採取する。`expected.json` には Next を含む対照のために、状態の列を持つ `trace` を足し、回帰テストが全状態を比べる。TLC が評価エラーを返すのに受理側で評価エラーになる行（`fold-independent-overflow`、`fold-singleton-error`）は、順序の拒否の文言でないことも検査する（status だけでは before の拒否と区別できない）。
- 走査は、判定を最初の文字列・record を見つけた時にだけ行う。今受理される fold はこの経路を通らない。判定の結果は prepared program の fold の node ごとに 1 回だけ計算する。prepared program に載らない複製された式（呼び出しの actual の中の fold）では、その都度計算する。
- 走査が最後まで進んだ後に整数 key の幅・Boolean などの key・model value の拒否が見つかる場合は、走査が最初に見つけた文字列・record の拒否を返す。今は最初に見つけた拒否を返すので、その文言と一致する。
- 入れ子の frame が自分の parameter として同じ番号を使う場合を数える規則は、plan のとおり読むと数える側に倒す。`fold_order.rs` にこの形の拒否を置く。

## 追記（2026-11-24）: レビューを受けた R1 の健全性の条件

R1 の和集合は、足す集合の要素を累積値の要素と比べる。整数と型付き model value の比較は評価エラーで、エラーになるかは TLC が畳む順序で変わる。rtlc は整列した順で評価するので、順序に依らない結果を返すという R1 の前提が成り立たなかった。

- 選ぶ直し: R1 の fold の結果の要素のどの 2 つも、両方向の `equal` でエラーにならないことを、畳んだ後に確かめる。エラーがあれば、従来の拒否を返す。結果が集合なので、どの順でも途中の比較はこの組のどれかになり、値とエラーの有無が順序に依らなくなる。
- 採らない案: 要素ごとの body の結果を先に全部評価して比較可能性を確かめる案。評価を 2 段にする変更が大きい。
- 判断の観点の追加: R1 は、結果の要素が比較できない組を持つ入力を受理しない。この拒否は、全要素が整数、Boolean、文字列の結果では確認を省く。record などの結果の確認は要素数の 2 乗の work を使う。この費用は iteration に測定値で記録する。
- `evaluation-budget` は `budget` と同じく置き換えない。

## 追記（2026-11-24, 2 回目）: 比較可能性の確認の範囲と対象

- 確認は fold の結果が集合の時だけ行う。初期値が整数などの場合、結果は集合ではなく、TLC も評価エラーにならない値を返す。
- 2 つの要素の確認は `equal` をやめ、対応する全ての field・入れ子の値を種類で比べる。`equal` は最初に違う field で止まるので、後ろの field の Int と Bool の食い違いが field の並びで見逃された。TLC が比べる field の順は IR からは分からないので、どの順でも評価エラーにならないことを、全ての対応する組で確かめる。長さが違う集合・関数は `equal` と同じく、その時点で不等として止める。
- 採らない案: field を正規の順に並べて `equal` を呼ぶ案。TLC の順と一致する根拠がない。
- 費用: 要素数 n の結果で組は n(n-1)/2、各組で比べる値の数の tick を使う。全要素が整数・Boolean・文字列の結果は確認を省く。費用は新しい run `0089c-*` に測定値で記録する。

## 追記（2026-11-24, 3 回目）

kind 比較の途中で予算が尽きた場合は、順序の拒否に置き換えず `budget` をそのまま返す。

## 追記（2026-11-24, 4 回目）

- 長さが違う集合は、各集合の中の全ての組を種類で比べてから通す。同じ長さの組の対応づけは変えない。
- 関数の key が全て文字列（record）の時は、field の名前で値を組にする。名前が共通しない field は比べない。key が文字列でない関数は従来どおり位置で組にする。
- 費用の測定は新しい run `0089d-*` に記録する。

## 追記（2026-11-24, 5 回目）: R1 の受理条件の開示と確認の事前登録

- 開示: R1（累積値への `\union`）の受理条件と、結果の比較可能性の確認（上の 2〜4 回目の追記）は、TLC の採取と実装の後に、レビューを受けて足した。結果を見る前に登録した条件ではない。最初の 3 形の予測値の記録は変えない。
- 集合どうしの確認は、2 つの集合の全ての要素を位置に依らず全ての組で種類を比べる。入れ子の集合は、片方の集合の中の組も、2 つの集合の間の組も確かめる。要素を位置で組にする対応づけは、record の field 名が並びを決めるため、TLC の intern ID の順と一致する根拠がない。関数の key が全て文字列（record）の時だけ、field 名で組にする。
- 確認の run `0089e-*` は、測定の前にここで条件を固定する。
  - grid（84 入力、v1・v2）: status と拒否の文言が `0089d-paths` と同じ。work の変化は許す。受理・拒否の数（56・21・7）が変わった場合は、変わった入力と理由を記録する。
  - fold_cost: 整数の fold の work・時間が `0089d-new-path` と同じ。record の work は同じか低い。集合を含まない入力なので、入れ子の集合の確認の費用は載らない。
  - 追加の回帰: 2 つの入れ子の集合の要素が field 名で共通する field の種類を食い違えるが、それぞれの集合の中では比べられる入力は、v1・v2 とも拒否する。同種の入れ子の集合は受理のまま。
- 入れ子の集合の確認は、組を作って溜めずに 1 組ずつ比べ、予算切れ・評価エラーで途中で止まる。

## 追記（2026-11-24, 6 回目）: 比較規則の書き下しと確認の再設計

- 指摘の対応を 1 件ずつ足すのをやめ、固定 TLC（`5a47802b5c391f59ecdd44117981f4ff8c0656ba`）の `compareTo` / `equals` から、種類ごとの規則を先に書き下す。表は iteration の「6 回目」に置く。`same_kinds` は、その表で「ある intern ID の順で評価エラーになりうる組」を拒否する形に直す。
- 直す点: 長さの違う集合は集合の内側の組だけを比べ、同じ長さの時だけ 2 つの集合の間の組も比べる。record 名の組の探索は 2 つの整列した列の併合にして、ステップごとに tick を使う。整数 key の関数・列は、先頭から比べ、最初に key か値が違う位置で止める。record と、key が文字列でない空でない関数・列は、長さが違っても拒否する。
- 事前登録（run `0089f-*`、測定の前）:
  - grid（84 入力、v1・v2）: status と拒否の文言が `0089e-paths` と同じ。受理 56・拒否 21・評価エラー 7、bounded は 0。work の変化は許す。変わった入力は理由を記録する。
  - fold_cost: 整数の fold の work が `0089e-new-path` と同じ。record の work は、併合の tick の分だけ増えるか同じ。時間は判定に使わず、記録する。
  - 回帰: 大きさの違う入れ子の集合 `{{1},{FALSE,TRUE}}` は受理（修正前は拒否）。同じ大きさの比べられない入れ子の集合は拒否のまま。列は最初の違いの位置で止め、`<<1,1>>` と `<<2,TRUE>>` は受理、`<<1,1>>` と `<<1,TRUE>>` は拒否。record と長さの違う列は拒否。record の field 名の併合が、field 数に比例して work を使うことを、2 つの幅の work の固定値で確かめる。
- ledger: 台帳の行は消さない。0089 の最初の実行の行が消えていることは、iteration の追記で開示する。
