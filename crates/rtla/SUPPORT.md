# rtlaの対応チェックリスト

チェック済みは構文解析とASTの保持に対応する範囲。評価・名前解決やモデル検査の対応とは別。未チェックは未対応またはこのcrateの対象外で、実装予定を約束しない。

## 対応済み

- [x] Module・Expression・TLC Configの型付きASTと、Rustのparse_module / parse_expression / parse_config。
- [x] MODULE、EXTENDS、変数・定数、通常の名前を持つ非再帰operator、関数定義、ASSUME、THEOREMの構文。
- [x] 既存operator語彙のprefix/infix/postfix定義、記号の定数・formal宣言、operator引数と修飾nonfix参照を既存のDefinition/Declaration/Apply/Identifierへ保持する。変数宣言は識別子に限る。
- [x] 算術・比較・論理、集合・tuple・record・関数、量化とtuple pattern、EXCEPTと@。
- [x] LET、IF、OTHERを省略したCASE、prime、UNCHANGED、ENABLED、temporal/fairnessの構文。
- [x] top-levelの引数なしnamed/unnamed INSTANCE、LOCAL、WITHの式置換、A!B!Op形式の修飾名をASTへ保持する。
- [x] infixの優先順位、括弧、コメント、文字列、bulleted junctionとoffside規則。Apalacheの`:=`もASTとして解析する。
- [x] 周囲のinfix/prefixの結合を変える非括弧のlabelをsyntax拒否する。優先順位を保つbody、label/body全体の明示括弧は保持する。
- [x] SUBSET/UNION/DOMAINのprefix優先順位は1.7.4の10..13を維持する。1.8.0の8/8・8/8・9/9とは区別する。
- [x] INIT/NEXT/SPECIFICATION、CONSTANT、INVARIANT/PROPERTY、constraintや置換の設定値の保持。実行の可否はcompilerが判断する。
- [x] native JSONLとホストimport不要のWasm ABI v1。
- [x] syntax/unsupportedを区別した位置付きエラー、整数表現と再帰・AST深さの上限検査。

- [x] 引数付きINSTANCE、LET内INSTANCE、nested module、RECURSIVE、LAMBDAを型付きASTへ保持する。
- [x] 固定Operators.javaの101 canonical operatorと有効alias、記号WITH左辺、label引数、一般のsubexpression selector。
- [x] parse_module_with_version / parse_expression_with_versionとJSON syntaxVersionで1.7.4/1.8.0の優先順位を選ぶ。Unicode operator・bullet・CASE・関数・量化aliasも解析する。
- [x] ASSUME/PROVEとproofの構造付きAST。BY/OBVIOUS/OMITTED、step階層とDEFINE/USE/HIDE/TAKE/WITNESS/PICK/CASE/HAVE/SUFFICES/QEDを保持する。
- [x] 不正構文をsyntax、解析・AST深さ64などの資源上限をunsupportedへ分類する。

## 非対応・対象外

- [ ] 外部モジュールの読み込み、名前解決、型/level検査、標準operatorの評価。
- [ ] ASTから検査IRへの変換、状態探索、invariantの評価。
- [ ] 過去の全SANY版の受理・拒否・診断との一致と、proofの証明検証。
- [ ] 全TLA+構文やSANYとの完全互換。

## 制約と根拠

- [x] `2^53-1`以下の整数はNumber、それより大きい整数はInteger.textへ原文を保持する。解析とASTの深さは64段まで。
- [x] 構文上保持できる設定でも、後段で未対応になる場合を保持する。

[README](README.md)、[公開境界](src/lib.rs)、[parser](src/parser.rs)、[回帰](tests/parser.rs)、[SANYとの対照](../../evals/iterations/0026-rtla-parser.md)を参照。

[355公式case・13 prefix対照の二版比較](../../evals/iterations/0068-rtla-compatibility.md)と[labelの回帰](tests/labels.rs)を参照。受理率を言語全体の対応率やAST全体の正しさへ読み替えない。

[記号operatorの構文と実行の比較](../../evals/iterations/0069-symbolic-operators.md)、[構文回帰](tests/symbolic.rs)と[固定入力](tests/fixtures/symbolic.json)を参照。記号定義の構文受理と、builtinとの衝突や高階operatorの実行可否を分ける。

syntaxVersionは優先順位表を選ぶ。文法・Unicodeは共通で、旧SANYのCASE/submoduleの不具合や廃止されたSetExcept文法を再現しない。proofの構文解析を証明の検証として扱わない。

[追加対応と独立比較](../../evals/iterations/0070-tla-coverage.md)を参照。構文受理・有限実行・証明の検証を区別する。
