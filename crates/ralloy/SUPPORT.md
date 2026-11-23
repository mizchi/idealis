# ralloyの対応チェックリスト

チェック済みは静的なAlloyサブセットを有限scopeで検査する範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] トップレベルsig、one/lone/some、名前が一意な二項fieldとmultiplicity。
- [x] `abstract sig`、`extends`、親が1つのsubset sig（`in`）、`sig A, B {}`、signature fact。子の明示scopeは親の上限を超えず、one/exactlyの子は親を広げる（0080、Alloy 6.2.0の答えで決めた）。
- [x] 3項・4項のfield（`A -> B`、`A -> B -> C`）と、1本の矢印の多重度`A m -> n B`。ownerのatomごとのtuple空間は64以下（0079）。
- [x] 4項までの関係の和・差・交差・積・join・domain/range制限・比較。型の交わらないjoinはAlloyと同じく空集合。
- [x] fact、引数なしpred、名前付きassert、run/check、全体scopeと型ごとの上限・exactly。
- [x] 関係の和・差・交差・積・join・転置・推移閉包・反射推移閉包・domain/range制限。
- [x] 等価・包含・論理結合、単一atomを束縛するall/no/some/one/loneとdisj。
- [x] none/univ/iden/Int/String。Int atomを扱うことと整数演算を区別する。
- [x] bit matrixで関係を評価し、有限構造を列挙してinstanceやcounterexampleを返す。
- [x] check_instanceで渡されたsig/fieldを固定し、scope・宣言・fact・性質を検査する。
- [x] matched / mismatch / invalid / unknownと、探索結果の各statusを区別する。
- [x] native JSONLとホストimport不要のWasm ABI v1。JVM・Z3・idealisの他crateに依存しない。

## Alloyとの差分テスト（0078）

- [x] 公式Alloy 6.2.0の答えをroracleの`CheckReport`として保存し、`cargo test`で再生する。group `core`は今の対応範囲だけで書いた13モデル・50 commandで、記録時に全commandがAlloyと一致した。
- [x] 評価器の人工故障6件（`one`/`lone`の数え方、閉包、反射閉包、転置、含意）を再生テストで検出する。

[評価0078](../../evals/iterations/0078-ralloy-alloy-oracle.md)を参照。

## 非対応・対象外

- [ ] 親が複数のsubset sig（`in A + B`）、`@field`、moduleの読み込み。
- [ ] 引数付きpred/function、setを束縛する高階量化、5項以上の関係、2本以上の矢印の多重度。
- [ ] 整数演算・cardinality、時間・varを使うAlloy 6の振る舞い。
- [ ] Alloy全体の構文・型検査、SAT符号化、節学習、一般的な対称性除去。
- [ ] 固定instanceの一致だけから未観測の実装履歴を保証すること。

## 制約と根拠

- [x] sig/field各8、sig scope各8、Int 0〜5 bit、全体64 atom等の上限とwork予算を検査する。
- [x] no-counterexampleはscope外の証明ではない。no-instance、unknown、エラーを合格に読み替えない。
- [x] noneは単項の空集合で、二項の空関係はnone -> noneで表す。

[README](README.md)、[parser](src/parser.rs)、[公開境界](src/lib.rs)、[固定instance](src/check.rs)、[評価](../../evals/iterations/0035-alloy-instance.md)を参照。
