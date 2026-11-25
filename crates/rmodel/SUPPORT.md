# rmodelの対応チェックリスト

チェック済みは、1つのモデルを構文・型で検査し、AlloyとTLA+の2つへ写して同じ意味になる範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] `sig`（scopeでちょうどの大きさ、1〜7）、静的な`rel`と状態の`var`。単項（`set`・`lone`・`one`・`some`）と二項（`A -> m B`）の関係（0091）。
- [x] `entity Account "説明"` は個体を持つ sort と任意の1行の用語定義を宣言する。型と意味は `sig` と共通。定義は Rust・TypeScript 型と用語集に反映する。[構文の照合テスト](tests/entity.rs)。
- [x] `enum Name = A | B`: 名前の付いた、ちょうど1つずつのatom。scopeを書かず、sigと同じく量化と関係の型に使う（0093）。
- [x] `fact`（varを参照しない設定の制約）、`init`、引数付きの`action`。actionは連言で、`v' = 式`の代入は1つのvarに1回まで、右辺と条件にprimeを書けない。代入しないvarと`rel`は変わらない（0091）。
- [x] `behavior 名前(引数) { guard { 条件 } update { v = 式 } }`。各ブロックは省略でき、両方を書く場合はguardが先。従来のactionと同じ型検査と同時更新を使う。[構文の照合テスト](tests/behavior.rs)。
- [x] 式: join（単項と二項、二項同士）、`->`、`+`・`-`・`&`、`none`、単項の`#`と絶対値7以下のリテラル。論理式: `in`・`not in`・`=`・`!=`・多重度・`no`・`not`・`and`・`or`・`implies`・`all`・`some`（0091）。
- [x] `invariant`（到達できる全状態）と`reachable`（到達できる状態がある）。関係の型と多重度は全状態で成り立ち、破る遷移は起きない（0091）。
- [x] Alloyへの写し方（ralloyのSATで有界、stepsのlasso）とTLA+への写し方（rtlcで完全、`rel`はInitで列挙）。

- [x] 参照評価器（`reference::evaluate`）: scopeの中を全列挙して、意味論の正本として判定を返す。空・成り立つ・破れる・到達できる・到達できない・未確定を区別し、破れる・到達できるには最短の深さを付ける（0097）。

- [x] sortの静的な型検査（集合と二項関係のsort、join・比較・代入のsortの一致）。多重度を破る遷移は組み込みのcheck `Declarations`で違反として報告し、違反した状態からは遷移しない（0099）。

- [x] 和のsort `type Party = Person + Org`: sig・enumをまとめた、自分のatomを持たないsort。和・比較・joinは部分型の関係で型を付け、Alloyでは`abstract sig`と`extends`、TLA+では子の集合の和に写す。1つのsigが属する和は1つまで、和の入れ子はない（0100）。

- [x] 値（0103）: `Int`（scopeの`Int = lo..hi`、範囲の外は組み込みのcheck `Scope`）、`Bool`、`record`（構造で等しい値、literal `R { f: e }`、field `x.f`）。値への関係は`one`・`lone`、算術`+ - *`と整数の比較は値が1つに決まる式だけ。TLA+へ写し、Alloyへは写さない（`alloy_supports`）。

- [x] `Nat` は `Int` の部分型で、`scope Int = lo..hi` の0以上の値を使う。引き算の値は整数で、負の値を Nat の変数や record フィールドに保存しようとすると `Scope` の違反になる。

- [x] 名前付きの有限列と値付き variant。列は最大長1〜4、構造化値のあるモデルでは各値型は64値まで。variant の各ケースは値なしまたは1値で、値型の再帰は拒否する。[有限列の例](../../fixtures/rcheck/structured/ordered-log.model) · [決済の例](../../fixtures/rcheck/structured/payment.model)。
- [x] `eventually`、量化できる `leads_to`、操作に対する弱／強公平性を TLA+ に写す。問いごとに公平性を上書きできる。[配信の評価](../../evals/iterations/0116-structured-temporal.md)。

- [x] `transition` は実行可能な各操作の前後を比較する。式の `before` と `after`、対照のプライムを型検査し、参照評価器と TLA+ で検査する。[出荷の例](../../fixtures/rcheck/structured/shipped-transition.model)。

- [x] 実装のためのbindings（`bindings`、0102）: JSONの形（atomは名前、Boolはboolean、recordはobject、二項関係は定義域のatomを鍵にしたobject）、JSON Schema（draft 2020-12）、Rustのtraitと型、TypeScriptの型とJSON Linesのdriver、Goのinterfaceと厳密なdecoder・`Serve`（0104、出力はgofmtの書式）、MoonBitのtraitと厳密なdecoder・`Server::handle`（0105、targetに依存しない）。参照評価器を1歩ずつ進める`reference::Machine`。

- [x] `validate`（0108）: parserを通らない構文木（核のIRのJSONなど）を、parserと同じ型検査と大きさの上限で検査する。Boolは名前が現れたときに足す。

## 照合

- [x] corpusの5モデル・20 checkで、ralloyのSAT・rtlc・Alloy 6.2.0・TLC 1.8.0の判定が、結果の前に書いた期待とすべて一致する。明示状態の深さはstepsの内側（0091）。

[評価0091](../../evals/iterations/0091-rmodel-language.md)を参照。

## 非対応・対象外

- [ ] 閉包、3項以上の関係、任意の時相論理式、三状態以上を直接参照する性質、module、sig の階層。
- [ ] Pklの仕様との結びつけ（0092）、Quint IRからの写し方。
- [ ] SATの有界な結果（stepsの外）を完全な検査として扱うこと。

## 制約と根拠

- [x] 未知の名前、fact での var の参照、transition の外にある式の prime、二重の代入、arityの違い、名前の隠蔽はエラーにする。
- [x] 2つの写し方は構文木だけを共有し、互いの出力を使わない。
- [x] 列挙範囲と演算の表現範囲を分ける。参照評価器は符号付き64ビットで表現できる算術を正確に評価する。TLA+ lowering は TLC に移植できる範囲 `-2147483647..=2147483647` を使う。型検査された式の範囲を各上限と比較し、超えうるモデルや lowering を拒否する。[0115 の結果](../../evals/iterations/0115-nat-validation.md)

[構文と型](src/syntax.rs)、[写し方](src/emit.rs)、[照合テスト](tests/engines.rs)を参照。
