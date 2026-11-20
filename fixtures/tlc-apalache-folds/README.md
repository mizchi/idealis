# Apalache fold の TLC 対照

2026-11-16 に TLC v1.7.4、revision `5a47802b5c391f59ecdd44117981f4ff8c0656ba` で採取したログ。jar の SHA-256 は `936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`。Apalache 0.56.1 の `tla2sany/StandardModules/Apalache.tla` の SHA-256 は `a9115b404e72c58fd085f81bc59cc918f8adac49feabfb57f8e285bc7ab5cfac`。採取前に両方を検証した。module 原本はリポジトリに含めず、隔離した scratch の各ケースディレクトリにだけ配置する。

入力は `evals/tlc-differential/corpus.ts` の `foldCases` / `foldOrderControls` / `foldRejections` / `functionOrderCases` の63件。すべてに `goldenFrontendParity` を宣言し、受理時の TS / Rust frontend の IR、拒否時の両 frontend の拒否、および native / Wasm を比較する。source と config を各ケースの `Differential.tla` / `Differential.cfg` にそのまま保存する。`TLC_JAR` は pinned jar の絶対パス、`TLC_TMP` はケースごとに分離した一時ディレクトリ。JVM は1つずつ起動し、value-order で追加する24ログを再採取した。既存37ログのバイト列は保持した。2026-11-17 に直接等価比較と fold body の等価比較の2ログを追加し、先行61ログは保持した。

```sh
java -Xmx256m -Djava.io.tmpdir="$TLC_TMP" -cp "$TLC_JAR" tlc2.TLC -workers 1 -nowarning Differential.tla
```

stdout と stderr を同じストリームへ保存し、次の置換だけを順に適用して `.log` に保存した。`temporary_directory` と `case_directory` は取得時に使った絶対パスで、パスの置換は Python の `str.replace`、残りは `re.sub` による。

| パターン | 置換後 |
| --- | --- |
| `str(temporary_directory)` | `<tlc-tmp>` |
| `str(case_directory)` | `<capture>` |
| `\[pid: \d+\]` | `[pid: <pid>]` |
| `\bon \d+ cores\b` | `on <cores> cores` |
| `\([^\n]*, (MSBDiskFPSet, DiskStateQueue)\)` | `(<host>, \1)` |
| `\b\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\b` | `<time>` |

ホスト説明の OS・architecture・JVM は置換し、FP set と queue の名前は残した。TLC のメッセージ、ソース位置、状態数・生成数、seed、fingerprint の行は変更していない。`expected.json` に source / config / 置換後 log の SHA-256、終了コード、判定、正常終了時の状態数・生成数、エラー時の文言と位置を記録する。
値は TLC の `x = ...` から PortableValue へ転記し、idealis の出力を期待値に使わない。値の対照は `Prop == x # x` の初期 invariant violation で取得する。Next の対照は2状態・3生成の完全探索。fixture の `unsupported` は TLC の失敗を表さず、TLC が値を返す入力を idealis の対応 fragment で拒否することを表す。

| 対照 | ケース | TLC / idealis |
| --- | --- | --- |
| 整数・Boolean | `fold-set-integers`, `fold-set-booleans` | 数値順、FALSE / TRUE の順が一致 |
| tuple / sequence・集合 | `fold-set-tuples`, `fold-set-sets`, `fold-set-recursive-values` | 要素数、その後に再帰的な辞書順が一致 |
| sequence の要素種 | `fold-sequence-values`, `fold-sequence-functions` | 文字列・record・関数・tuple・集合・Boolean・整数を先頭から処理 |
| 引数順・空・scope | `fold-sequence-left`, `fold-empty-set`, `fold-empty-sequence`, `fold-let-operator`, `fold-nested`, `fold-bound-capture`, `fold-next-action` | 累積値が第1引数で、列は先頭から処理する。空では body を評価せず initial を返す |
| 通常の名前解決 | `fold-own-ApaFoldSet`, `fold-own-ApaFoldSeqLeft` | EXTENDS Apalache のない同名の通常 operator |
| 再帰的な対応範囲 | `fold-unsupported-{string,record,function}-{direct,tuple,set}` の9件 | 旧名を保持する。整数 key の function 3件は受理へ変更し、string / record 6件は種類別の理由を明示して拒否 |
| 混合型 | `fold-set-mixed-kinds` | 両方とも評価エラー |
| fragment 制限 | `fold-reject-lambda`, `fold-reject-operator-parameter`, `fold-reject-recursive`, `fold-reject-other-apalache` | TLC は値を返す。frontend は拒否 |
| 不正な形式 | 残り7件の `fold-reject-*` | TLC の input error と frontend 拒否。正常の意味論一致には数えない |

sequence は先頭から処理し、文字列・record・関数を含む任意の要素種を扱う。集合 fold は TLC v1.7.4 (`5a47802`) の順序に合わせ、整数、Boolean、tuple / sequence、集合、整数 key の関数と、それらの再帰的な組み合わせを扱う。整数は数値順、Boolean は FALSE / TRUE、tuple / sequence と集合は要素数の後に再帰的な辞書順、関数は domain 要素数の後に各 key / value を交互に比較する。集合要素内の全整数 function key の最大値と最小値の差が `i32::MAX` を超える場合は拒否する。TLC の interval / 明示 domain 間の32-bit減算で順序が反転するためで、IR は元の domain 表現を区別しない。primitive 整数や関数の値にこの幅制限は課さない。domain が `1..n` の関数は既存の値表現どおり sequence として扱う。文字列・record をどの深さでも含む集合は、IR に TLC の文字列・field 名の intern ID がないため、それぞれ理由を明示した評価時エラーで拒否する。非整数 key のその他の関数は従来の `Unsupported TLA input: set fold order for strings, records or functions` を維持する。model value は従来どおり未対応で、異なる値種の比較が必要な集合も拒否する。順序に依存しない sequence fold は要素種を制限しない。

| value-order の対照 | TLC の値・結果 | idealis |
| --- | --- | --- |
| `fold-set-functions` | `0↦-1`、`{0↦0,3↦0}`、`{0↦9,2↦9}` の順 | domain の大きさ、各 key/value の交互比較で一致 |
| `fold-set-recursive-functions` | tuple 内の集合内の関数も同じ順 | 再帰的に受理 |
| `fold-set-interval-functions-safe` | key が -2147483647 と 0。差はちょうど i32::MAX | 受理 |
| `fold-set-interval-functions` | `<<1>>` が key -2147483648 の関数より先 | key 幅超過を拒否し、TLC の減算 overflow を再現しない |
| `fold-set-explicit-functions` | 明示 domain 同士では key -2147483647、1 の順 | IR が表現を区別しないため、同じ幅制限で拒否 |
| `fold-set-wide-recursive-functions`, `fold-set-wide-single-function` | 入れ子の別関数と単一関数内の幅超過 | いずれも拒否 |
| `fold-set-wide-integers`, `fold-set-wide-function-values`, `fold-set-wide-tuple-values` | 整数値 -2147483648、2147483647 の順 | 値を key と混同せず受理 |
| `fold-set-strings`, `fold-set-strings-reversed` | `a,b,A,aa,B,""` と `a,b,"",B,aa,A` | intern ID がないため両方を拒否。辞書順を推測して採用しない |
| `fold-set-record-fields`, `fold-set-records` | zeta が alpha より先。同じ field の record は値順 | field 名の intern ID がないため拒否 |
| `fold-set-nested-strings`, `fold-set-sets-of-strings`, `fold-set-functions-with-strings`, `fold-set-functions-with-records` | tuple・集合・整数 key 関数内の文字列と record | 再帰的に理由付きで拒否 |
| `fold-set-boolean-function-keys` | FALSE を key とする関数 | 従来の function 拒否文言を維持 |
| `function-initial-choice` | 最初の反例は `{0↦0,3↦0}` | 有限集合からの初期状態選択で一致 |
| `function-quantifier-order` | FALSE の先行評価で空列 Head を評価しない | 短絡して FALSE |
| `function-comparison-error` | 最初の key が等しく、その値の Boolean / 整数比較でエラー | 後続 key へ進まず評価エラー |
| `function-set-equality`, `function-set-membership` | TRUE | 等価性と所属を維持 |
| `function-equality-entry-error`, `fold-equality-entry-error` | 共通 key 0 の TRUE と 0 の等価比較で終了コード75の評価エラー | 後続 key 2 / 3 の不一致より前に評価エラー |

関数の順序は pinned TLC の [FcnRcdValue](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/FcnRcdValue.java) の `compareTo` / `compareOtherInterval` / `compareToInterval` と照合した。文字列と field 名は [StringValue](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/StringValue.java)、[RecordValue](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/RecordValue.java)、[UniqueString](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/util/UniqueString.java) の intern ID 比較を基準にする。tuple と集合は引き続き [TupleValue](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/TupleValue.java) と [SetEnumValue](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/SetEnumValue.java) に合わせる。

比較の影響調査: `Arena::total` は `ordered` による集合の正規化、`function` による関数 key の正規化、`comparable` の型検査の打ち切り、RecordSet の field 整列で呼ばれる。文字列だけの field 整列は今回の関数比較変更の影響を受けない。`Domain::values` はこの集合順を使うので、状態の所属による列挙、Bind、有限量化、集合内包・写像・関数生成、ENABLED の早期終了へ伝わる。集合演算・powerset・function set の列挙、membership の候補順と型エラー、set equality の対応位置にも間接的に伝わる。最初の反例、予算境界、量化・所属の短絡で観測できる。TLA+ の CHOOSE 式は両 frontend とも未対応で、IR の `Step::Choose` は状態変数の所属による列挙を表す。CHOOSE 式の対応は追加しない。

`Arena::equal` の関数比較は要素数を確認し、各 key とその値を交互に比較する。最初の不一致で FALSE、比較不能な値では後続 key へ進まず評価エラーを返す。`select` の key 検索は変更しない。全比較可能値の等価性・所属は維持する。内部正規化は変わるが、返却値は `Arena::portable` から `Value::normalize` の JSON 表現順で再正規化するため、set/map 自体の表示順はそのまま。選ばれる反例の値や関数を key とした検索・短絡は変わりうるので、上の fold 以外の golden でも検査する。既存 corpus の変更前221受理入力は、変更後もエラー・trace・work を含む完全応答が一致した。既存37ログと保存 run は変更していない。旧 function 拒否対照3件の受理と string/record の理由の変更は、この対応範囲変更として明示する。

等価比較は同じ revision の [FcnRcdValue.equals](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/FcnRcdValue.java#L167-L225) をソースで確認した。両 domain が整数 interval なら、空 domain はともに空、非空では要素数と先頭 key が一致すれば全 key が一致し、その後の値の順次比較も展開済みの key/value 比較と同じ結果になる。片方だけ interval の両分岐は要素数を確認し、各整数 key と値を交互に比較するため、同じ展開表現に一致する。これは比較順の確認であり、`compareTo` の減算 overflow による順序問題の扱いは変更しない。[RecordValue.equals](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/RecordValue.java#L96-L114) は field 名と値を交互に、[TupleValue.equals](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/TupleValue.java#L82-L96) は要素順に比較する。[SetEnumValue.equals](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/value/impl/SetEnumValue.java#L105-L125) は正規化後に要素数と対応要素を順に確認しており、key と値を二巡する同種の問題は見つからなかった。集合の等価比較は変更していない。
