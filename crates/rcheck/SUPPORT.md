# rcheckの対応チェックリスト

チェック済みは、rmodelのモデルの問いを、登録した検査器が宣言した範囲で答え、用語集の語で返す範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] 問い: モデルの`invariant`・`reachable`と、組み込みの`Declarations`（0101）。
- [x] 検査器の宣言（`supports`）: 参照評価器は関係の取りうるtupleが20以下ならcomplete、明示状態（rtlc）はcomplete、SAT（ralloy）はstepsまでのbounded（0101）。
- [x] 答え: 成り立つ・破れる・到達できる・到達できない・空・未確定・エラーと範囲、破れる・到達できるときの最短の深さ。boundedの成り立つ・到達できない、予算切れ、エラーをcompleteの答えとして扱わない（0101）。
- [x] 選び方: `auto`はcompleteで答える最初の検査器、名前の指定、`all`は全検査器と食い違いの報告。boundedの否定の答えは、bound以上の深さの反例とは食い違わない（0101）。

- [x] 予算: 参照評価器は、到達した状態だけでなく、調べた設定と初期状態の候補も予算に数え、組み合わせを1つずつ作る（0101、0100のcrashした2 seed）。

- [x] 値（0103）: SATは整数とrecordのモデルに対応しないと宣言する。明示状態は、否定の答えの前に組み込みの`Scope`を検査し、状態が整数の範囲の外へ出たら未確定と答える。

- [x] 実装の照合（`conform`、0102）: 参照評価器を正本に、設定ごとに実装の初期状態がモデルの初期状態であること、到達する各状態で各action instanceの受け付けと次の状態が一致することを、幅優先で確かめる。答えは一致・bounded（設定の上限）・不一致（最短のcommandの列、期待と実測）・invalid（JSON Schemaに合わない答え）・空・未確定（呼び出しの予算）・エラー。driverは同じプロセス（生成したRustのtrait）か、JSON Linesの外部プロセス（生成したTypeScriptの`serve`）。
- [x] `generate`: モデルからRustのtrait、TypeScriptの型とdriver、Goのinterfaceとdecoder（0104）、MoonBitのtraitとdecoder・`Server::handle`（0105）、JSON Schema、到達できる状態を生成する（0102）。JSON Schemaの検査は、生成するキーワードだけを扱う`schema::validate`で、知らないキーワードはエラーにする。

- [x] 核のIRのJSON（version 1、`ir`、0108）: rmodelの型検査前の構文木。式・論理式は`op`で区別する木、知らないtag・欠けたfield・知らないfieldは拒む。`rcheck generate <model> ir`で書き、`.json`の入力は`rmodel::validate`で型検査してから検査器に渡す。adapterが写せなかった部分は`omitted`。
- [x] `entity` の用語定義を IR の任意フィールド `entity_definitions` と `generate glossary` に写す。定義を持たない IR は従来の形式で読める。[IR の往復テスト](tests/ir.rs)。
- [x] `rcheck dead-code` は参照評価器で、到達する状態で成立するガードを操作ごとに調べる。未確定、空、IR の欠落をデッド扱いしない。[到達性の CLI テスト](tests/dead_code.rs)。
- [x] `rcheck lean-oracle` は、参照評価器が完全探索できた非空のモデルから設定と状態を抽出し、別の Lean ファイルに JSON で渡す。偽の観測と実行エラーを区別する。[入出金の命題](../../formal/rmodel/Bank.lean) · [oracle の CLI テスト](tests/lean_oracle.rs)。
- [x] adapterの差分テスト（0108）: roracleで、rmodelの文の判定を基準に、adapterのIRの判定を照らす。unsupported・omittedはunknown、拒まれたIRはerrorで、一致に数えない。

## 照合

- [x] corpus（`fixtures/rmodel`・`spec/models`）の32 claimで、`auto`の答えがclaimと一致し、`all`の3つの答えが食い違わない（0101）。

## 非対応・対象外

- [ ] 明示状態とSATのwitness（trace）を核の値へ戻し、参照評価器で検査し直すこと。
- [ ] `auto`で、選んだ検査器が未確定のときに次の検査器へ移ること。
- [ ] SMTによる帰納法などの新しい形式化の検査器。
- [ ] 値の集合、実装の状態の一部だけを観測すること。

## 制約と根拠

- [x] 参照評価器が対応しない大きさでは、TLA+の写し方も初期状態を`SUBSET`で数え上げるので、明示状態の予算が尽きやすい。そのときは未確定と答える。

[検査器と選び方](src/lib.rs)、[登録した検査器](src/checkers.rs)、[実装の照合](src/conform.rs)、[テスト](tests/registry.rs)・[実装](tests/domain.rs)を参照。
