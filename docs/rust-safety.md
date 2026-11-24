# TLA+ の有限モデルを Rust で探索する

検査器は独立した[`rtlc` crate](../crates/rtlc)に置く。`rtlc::ir`が公開IR、`SafetyProgram::check` / nativeの`check_parallel`が実行API。idealis-coreはこのbackendを利用し、既存のRust APIとJSONL/Wasm ABIを維持する。TLA+ソースを読むTypeScript frontendはidealis側にある。

独立した[`rtla` crate](../crates/rtla)は、TLA+／TLC設定を型付きASTへ解析する。[`rtla-compiler`](rust-frontend.md)が名前解決とIR生成を担当し、`compileTlaSource` / `checkTlaSource`でRustだけの経路を呼べる。既存の`compileTlaSafety`はtla-ts parserを継続利用する。rtla単独のnative/Wasm解析は`just rtla-test` / `just rtla-browser`で検査できる。

`compileTlaSafety` は tla-ts の parser が返す構文木から、有限モデル用 IR を作る。Rust コアが式を評価し、Init / Next の状態生成、BFS、重複排除、不変条件・deadlock の検査、反例復元を実行する。temporal property がある場合は、有限グラフと否定した性質の generalized Büchi automaton の積から、受理する SCC を探す。全状態グラフを TS で先に列挙する必要はなく、探索中に JS へ戻らない。

source APIの`irVersion?: 1 | 2`は省略・明示1で既存のsafety IR v1を返す。明示2では両frontendが式定義を共有するv2を生成し、rtlcが定義表とcallerのlexical frameを持つ遅延引数を検証・評価する。callと引数の要求へ各1 workを課金し、値はmemoizationしない。actionはinlineを維持し、明示v2は再帰expressionを扱う。v1拒否からの自動再試行は行わない。wire契約と定義順は[IR v2](safety-ir-v2.md)を参照。request envelopeやWasm ABIのversionとは別の指定である。

```ts
import { compileTlaSafety } from "@mizchi/idealis/tla";
import { checkSafety, createPortableWasm } from "@mizchi/idealis/portable";

const core = await createPortableWasm(wasmBytes);
const program = compileTlaSafety({
  source: `---- MODULE Counter ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Safe == x < 3
====`,
  config: "INIT Init\nNEXT Next\nINVARIANT Safe",
});
const result = checkSafety(core, program, {
  maxDepth: 10,
  maxStates: 100,
  maxWork: 10000,
});
// invariant-violation、x=0,1,2,3 の最短反例。
```

ブラウザでは `wasmBytes` を fetch で読み込める。compiler は optional peer の tla-ts を使い、その公開エントリを browser bundle にする場合は既存の [browser test](../tests/browser.spec.ts) と同じ `node:zlib` → `fflate` の alias と未使用の `bun:jsc` の除外が必要。生成した `modelt.safety-program` v1 の JSON を保存すれば、実行側は tla-ts やこれらの alias を必要としない。Rust native の `idealis-core` binary は `{"version":1,"request":{"kind":"check-safety","program":...,"bounds":...}}` を1行の JSON として受け取る。Rust library は deserialize した `SafetyProgram` に `check(SafetyBounds)` を呼ぶ。native と Wasm は同じ探索コードを使う。

## nativeの並列探索

native libraryの`SafetyProgram::check_parallel(bounds, workers)`は、同じ深さの状態をworkerへ分配する。JSONL binaryでは`check-safety-parallel`を使う。`workers`は1〜256の整数の上限で、1は既存の逐次経路になる。既存の`check` / `check-safety`も逐次経路を使う。

```ts
const command = {
  version: 1,
  request: {
    kind: "check-safety-parallel",
    program,
    bounds: { maxDepth: 100, maxStates: 10000, maxWork: 1000000 },
    workers: 4,
  },
};
```

`program`には`compileTlaSafety`が返したIRを入れ、`JSON.stringify(command)`をbinaryのstdinへ1行で送る。新しいpublic helperを追加せず、既存のJSONL requestとRustのmethodでworker数を指定する。初期状態・assumption・初期propertyは逐次で検査する。action property・temporal property・fairnessを含むモデルも逐次へfallbackする。scalarの四則・比較・論理・区間所属だけの軽いモデルと、幅16未満のfrontierは逐次実行する。複合値操作・量化・Bind/Chooseを含む広いfrontierでpoolを起動する。

state invariantを持つsafety探索では、既存の値を共有し、worker専用の値をcommit時にcanonical化する。親・action・状態の列挙順を保ち、同じ層の処理が終わってから次の層へ進む。十分な予算がある場合の状態数・生成数・最短safety反例は逐次と一致する。hash衝突時には状態全体を比較する。

workはworker専用のcreditから消費し、256 tick分を共通予算から予約する。予約とworker間の未使用creditの移動だけを同期する。全体の残り予算が少ない場合も、未使用creditを使い切れる。

maxStatesとmaxDepthは全体に適用する。maxWorkも全worker共通だが、workerが重複して生成した値と、停止までの先行評価にも課金する。canonical化後の一意な新状態だけを、元の候補順でinvariant検査する。そのためwork量と予算境界での部分結果は逐次と同一とは限らない。返却するworkは上限を超えず、未処理の候補や失敗を残したcompleteを返さない。状態batchは最初64状態/taskで、生成候補数から16〜128状態へ調整する。1 taskあたり1,024候補を目標にし、task数は指定worker上限とfrontier幅に応じて選ぶ。これは1状態から生成する候補数や複合値全体への厳密なメモリ上限ではない。

このrequestはnative専用。Wasmは明示的に拒否する。ブラウザ側は既存の逐次Wasmと、独立モデルを分配するpoolを使う。[方式と制約](tlc-parallel-adoption.md)を参照。

## アクションのcoverage

Rustの`check_with_coverage`と、portableの`checkSafetyCoverage(core, program, bounds)`で、通常のresultに加えて逐次BFSのbranch計数を得る。sourceからはcompileTlaSourceで作ったprogramを渡す。初期/遷移branchの順と同名の行を保ち、無効だったattempt、完成候補、保存した新規state、workを区別する。[計数と制約](../crates/rtlc/README.md#アクションのcoverage)を参照。式単位と並列のcoverageは未対応。

## 対応範囲

基準は TLC v1.7.4 (`5a47802`)。state は signed 32 bit 整数、真偽値、文字列、有限集合、有限関数を扱う。tuple / sequence は1始まりの有限関数、record は文字列をキーにする有限関数として等価性を判定する。関数の等価比較は要素数を確認した後、正規化済みの各 key とその値を交互に比較し、最初の不一致で FALSE、比較不能な値で評価エラーを返す。tuple / sequence は添字順、record は既存の field 正規化順で同じ判定を行う。値は immutable な arena に intern し、state はその ID を保持する。hash が衝突しても、型付き値の完全一致で重複排除する。

主な演算は次の通り。

- 非再帰 operator / 関数定義、LET、有限の全称・存在量化、tuple pattern。
- 加減乗算、比較、論理、IF、CASE。CASE は式・action ともソース順で最初の真の arm を選び、OTHER を省略できる。OTHER がなく全条件が偽なら、その CASE の評価時に TLC と同じ未一致エラーとソース位置を返す。未評価の分岐ではエラーにしない。整数 overflow はエラー。
- 集合の列挙・内包・写像、和・積・差・部分集合、SUBSET、UNION、Cardinality。
- 関数・record の生成、DOMAIN、適用、所属、EXCEPT。複数の更新は順に適用し、@ は各更新の直前の値。存在しない EXCEPT path では右辺を評価しない。
- sequence の Len / Head / Tail / Append / SubSeq / 連結、有限集合を要素にする Seq への所属。
- 変数全体への代入、有限領域からの選択、UNCHANGED、存在量化による action の列挙。

defining module が直接 `EXTENDS Apalache` を指定するときだけ、`ApaFoldSet(Op, v, S)` と `ApaFoldSeqLeft(Op, v, seq)` を扱う。`Op` は呼び出し位置で可視のtop-level / LET定義のoperator名またはLAMBDAで、零 arity の引数をちょうど2つ持つ必要がある。累積値が第1引数、要素が第2引数。foldの専用frameでbound slotを2つ使い、v1では呼び出し位置でbodyを展開する。v2では同じslotをactualとして共有expression定義をcallする。Rust frontendはLAMBDAとoperator parameterを静的に特殊化し、再帰callbackはv2を要求する。他のApalache operatorはチェックリストの範囲に従う。`EXTENDS Apalache` がなければ fold 名も通常の名前解決に従う。展開深さ・bound variable の上限は変更しない。

sequence は先頭から処理し、文字列・record・関数を含む任意の要素種を扱う。集合 fold は TLC v1.7.4 (`5a47802`) の順序に合わせ、整数、Boolean、tuple / sequence、集合、整数 key の関数と、それらの再帰的な組み合わせを扱う。整数は数値順、Boolean は FALSE / TRUE、tuple / sequence と集合は要素数の後に再帰的な辞書順、関数は domain 要素数の後に各 key / value を交互に比較する。要素が 2 個以上の集合では、集合要素内の全整数 function key の最大値と最小値の差が `i32::MAX` を超える場合は拒否する。TLC の interval / 明示 domain 間の32-bit減算で順序が反転するためで、IR は元の domain 表現を区別しない。primitive 整数や関数の値にこの幅制限は課さない。domain が `1..n` の関数は既存の値表現どおり sequence として扱う。文字列・record をどの深さでも含む集合は、IR に TLC の文字列・field 名の intern ID がないため、要素が 2 個以上で結果が順序に依存しうる fold を、それぞれ理由を明示した評価時エラーで拒否する。順序に依存しないと IR から分かる fold は受理する。要素が 1 個以下の集合（種類を問わない）、body が要素を読まない fold、body が累積値に `\union` で集合を足す fold（足す集合と IF の条件は累積値を読まない）である。 累積値は、足す `\union` の直接の被演算子でなければならない。`acc \union ({elem} \union {"x"})` は受理し、`(acc \union {elem}) \union {"x"}` は受理しない。要素が2個以上で文字列・record を含む集合は、要素の中に比べられない組（field の種類の食い違いなど）があれば、fold を受理する前に従来の拒否を返す。累積値に足す fold の body が評価エラーになる場合と、結果の要素に比較できない組（整数と型付き model value など）がある場合は、TLC が最初に報告するエラーが順序で決まるため、従来の拒否を返す。置き換えない例外は、要素が 1 個の集合（body のエラーをそのまま返す）、`Unsupported TLA input` で始まる別の拒否、予算切れ（`budget`・`evaluation-budget`）である。要素が 2 個以上の集合 fold では、非整数 key のその他の関数は従来の `Unsupported TLA input: set fold order for strings, records or functions` を維持する。要素が 2 個以上の集合 fold の model value は未対応で、異なる値種の比較が必要な集合も拒否する。[pinned TLC ログ](../fixtures/tlc-apalache-folds/README.md)を対照にしており、Apalache 全体との互換性を表さない。

primed variable は、同じ分岐で代入した後に読み取る。部分的な関数更新を左辺に置く式から、未代入の変数を逆算しない。ENABLED は列挙可能な action に限り、fairness の subscript は候補ごとに評価できる必要がある。未代入の next variable に任意の値を補わない。

設定は INIT / NEXT または SPECIFICATION に、INVARIANT / PROPERTY / CONSTANT / CHECK_DEADLOCK を組み合わせる。SPECIFICATION は初期述語、全 state variable を列挙した subscript の `[][Next]_vars`、WF / SF の連言を扱う。PROPERTY は state / action predicate、論理結合、always / eventually / leads-to を扱う。action を含む一般の temporal property は `[]<>A` / `<>[]A` の範囲に限る。定数の ASSUME も確認する。

Rust compilerはselected operatorと依存をlowerする。全SANYの意味・型・level検査やproofの検証は行わない。無限集合の列挙、state依存の時間論理domain、一般のsymmetryとVIEWは未対応。Rust固有の追加構文・高階・再帰の範囲は[frontendの契約](rust-frontend.md)に従う。

Rustのsource経路は定数有限domain上のSPECIFICATIONの量化fairnessと、SelectSeq / SortSeq / Assertを追加で扱う。callbackは名前またはLAMBDAを使う。再帰callbackはv2を要求する。SortSeqはTLCの挿入順・比較順を保持し、反復・比較でworkを消費する。Assertはmessageも評価する。[追加の契約](../crates/rtla-compiler/README.md#標準演算と量化された公平性)を参照。既存のcompileTlaSafetyへ同じソース対応を追加した意味ではない。

Rustのsource経路はmodulesを受け、IR v1/v2でEXTENDS・引数付きINSTANCE・LET内INSTANCE・nested module・LOCAL・WITH・設定置換を解決する。ファイルとネットワークは読まない。[frontendの契約](rust-frontend.md)を参照。

frontend の上限は、変数256、分岐4096、分岐あたり256 step、展開深さ40、bound variable 40。実行側も IR のフィールド・参照・式を検証する。複合値の深さは64で、それを超える state の生成はエラー。探索中は状態と親 index を保存し、temporal property がある場合だけ遷移グラフも保存する。

## 結果と予算

`complete` は reachable graph が閉じ、指定した不変条件・deadlock・property の検査が完了したこと。temporal property は fairness を満たす無限の振る舞いを対象にする。fairness を満たす振る舞いの存在を別途保証する結果ではない。`bounded` は探索予算切れ、`empty` は初期状態がないこと。いずれも合格へ読み替えない。

違反の区別は次の通り。

- `invariant-violation` / `deadlock` / `assumption-violation` は既存の違反名と trace を返す。
- `property-violation` は初期 state property または action property の違反。TLC がこれらを liveness violation と表示する場合も、この API では分ける。
- `liveness-violation` は無限に繰り返せる lasso を返す。`loopStart` がループの先頭 index、`loopAction` が最後の状態から先頭へ戻る遷移。

`PROPERTY P` の P が state predicate なら初期状態で検査する。`PROPERTY AlwaysP` の本体が `[]P` なら invariant として扱う。最短性を保証するのは BFS の safety trace。liveness の lasso は最短とは限らない。不完全な状態、型違い、overflow、不正 IR は例外となる。定数 FALSE の invariant / property も TLC と同じく探索前のエラーで、初期集合が空でも拒否する。

`maxDepth` / `maxStates` / `maxWork` は必須。`maxTemporalNodes` は property ごとの積グラフの上限で、既定値は `maxStates`。積の予算切れは `bounded` / `reason: "temporal"` になる。work は分岐の処理、binding の列挙、複合値の生成、automaton の展開・SCC 検査などで共有する。命令数、wall time、厳密なメモリ量の上限ではない。自動 timeout は追加しない。maxDepth の境界でも successor が既訪問状態だけなら complete にできる。

liveness では各状態に stuttering の自己遷移を追加する。WF は継続して有効な action、SF は無限回有効になる action の実行を要求する。deadlock は元の Next で判断し、自己遷移を追加して deadlock を隠さない。

trace の values は program.variables の順の PortableValue。set / map / record は正規化し、1..n を domain にする関数は list として返す。action は分岐の operator 名、初期状態は null、暗黙の自己遷移は `<stuttering>`。Driver 用 oracle にする場合は、仕様の値・操作名から実装の Action / Observation へ明示的に map する。

```sh
just safety-test
just safety-tlc java /path/to/tla2tools.jar
just safety-eval .artifacts/rust-safety-run
just safety-replay .artifacts/rust-safety-run
just complex-eval .artifacts/complex-run
just complex-replay .artifacts/complex-run
just complex-tlc /path/to/java /path/to/tla2tools.jar .artifacts/complex-tlc
just complex-tlc-replay /path/to/java /path/to/tla2tools.jar .artifacts/complex-tlc
```

[0009 の計画](../evals/plans/0009-rust-safety.md) に判定比較と性能条件を記録する。tla-ts 全体との性能比は、同じ仕様を扱う今回の fragment の範囲で解釈する。frontend・Wasm 起動・native process 起動を分けて測る。

[測定・差分検査・採用判断](../evals/iterations/0009-rust-safety.md) に、初回の改善不足、profile、literal decode と状態の連続格納の結果を記録している。

複合値・temporal の条件は [0012 の計画](../evals/plans/0012-collections-temporal.md)。検査は native / Wasm / tla-ts / TLC で比較し、反例の state と loop を固定した別の仕様で再生する。強制した遷移が元の Next または stuttering であること、全 trace state が到達すること、fairness と性質の違反を確認する。誤った性質を要求する対照は違反する必要がある。原文の [Channel](../fixtures/channel/Channel.tla) も検査する。

[0012〜0014 の評価結果](../evals/iterations/0012-collections-temporal.md) に、対応検査と性能の改善・未達条件を記録している。`safety-eval` の既定 plan は0014で、任意の既存 plan を第2引数に指定できる。

[TLCとの直接差分検査](../evals/iterations/0015-tlc-differential.md) で、空区間の所属、部分集合の短絡、型集合の評価順、文字列演算を修正した。文字列の Head / Append はこの TLC version の動作に従う。TLA+ source の文字列 literal はこの版の字句範囲に限定し、範囲外の文字は frontend が拒否する。UTF-16 の途中で surrogate pair を分割する結果は portable string として表現せずエラーにする。

Rust frontendとrtlcの追加対応は[crateのチェックリスト](../crates/rtla-compiler/SUPPORT.md)を参照。model value、有限CHOOSE、整数の\div / % / ^、CONSTRAINT / ACTION_CONSTRAINT、PROPERTY内の定数有限domain上の時間論理量化を扱う。既存TS frontendの対応範囲とは区別する。
