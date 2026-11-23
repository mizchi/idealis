# ralloyの対応チェックリスト

チェック済みは静的なAlloyサブセットを有限scopeで検査する範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] sig: トップレベル・`abstract`・`extends`・親が1つの`in`、one/lone/some、`sig A, B {}`、signature fact。子の明示scopeは親を超えず、one/exactlyの子は親を広げる（0080）。
- [x] field: 名前が一意で4項まで、`A m -> n B`。fact、assert、run/check、全体scope・型ごとの上限・exactly、`for`なしの既定3（0079、0080）。
- [x] 式: 4項までの和・差・交差・積・join・domain/range制限、転置・閉包、比較・論理結合、atomを束縛する量化とdisj、none/univ/iden/Int/String、引数付きpred/funとbox join（0079、0081）。
- [x] 整数: リテラル、`#e`、`plus`などの算術、比較。Int型の関係は値の和になり、bit幅の2の補数で桁あふれする（0082）。
- [x] 入れ替えられるatomの対称性を、lex-leaderで探索の途中で除く。2列以上のfieldとsubset sigも扱う（0083、0084）。
- [x] sigの階層も整数式もないモデルで、fieldのslotの幅が24 bitを超えるcommandは、関係をSATへ符号化してralloy内のCDCLで解く（0085）。
- [x] 有限構造の全列挙、`check_instance`、matched/mismatch/invalid/unknownとstatusの区別、native JSONLとホストimport不要のWasm ABI v1。

## Alloyとの差分テスト

- [x] 公式Alloy 6.2.0の答えをroracleの`CheckReport`として保存し、`cargo test`で再生する。5 group・37モデル・174 commandすべてで一致する（0078〜0085）。

[評価0078](../../evals/iterations/0078-ralloy-alloy-oracle.md)から[0085](../../evals/iterations/0085-ralloy-sat-backend.md)を参照。

## 非対応・対象外

- [ ] 親が複数の`in`、`@field`、module、`set`などの引数、高階量化、5項以上の関係、`sum`、整数を関係として使う式、0による割り算、時間・var。
- [ ] Alloy全体の構文・型検査、階層や整数式を含むモデルのSAT符号化。
- [ ] 固定instanceの一致だけから未観測の実装履歴を保証すること。

## 制約と根拠

- [x] sig 16・field 8、sig scope各8、Int 0〜5 bit、全体64 atom、fieldのtuple空間64とwork予算を検査する。
- [x] no-counterexampleはscope外の証明ではない。no-instance、unknown、エラーを合格に読み替えない。

[README](README.md)、[parser](src/parser.rs)、[公開境界](src/lib.rs)、[固定instance](src/check.rs)を参照。
