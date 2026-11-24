# rtla-compiler

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

rtlaの型付きModule/Configを、rtlcのSafetyProgramへ変換するRust frontend。parserとcheckerを別crateに保ち、両者の公開契約をつなぐ。Nodeやsolverには依存しない。

既存語彙のcustom記号operatorは通常の定義として解決する。`left %% right == ~left`、`-. value == ~value`、`value ^+ == ~value`とnonfix呼出しをIR v1/v2で扱う。builtinやimport済み記号のbinding衝突は、未使用定義・LET/formal宣言を含めて拒否する。[固定した構文・TLC比較](../../evals/iterations/0069-symbolic-operators.md)を参照。高階operatorは静的な名前またはLAMBDAを特殊化し、operatorをstateへ格納する一般の値とは区別する。

`compile(&module, &config)`は名前解決、定数とoperatorの展開、action列挙、temporal/fairnessを変換する。既存TS frontendの有限fragmentを基礎に、Rust経路では[Quint生成物](../../docs/quint-generated.md)の限定した構文も扱う。明示したユーザーモジュール集合と引数なしINSTANCEにも対応する。stateへ格納するoperator値とv1の再帰operator、無限集合の列挙、一般のsymmetry、VIEWは扱わない。解析はrtla、実行時の型・IR検査と探索はrtlcが担当する。

省略・明示v1はJSONのIR nodeを組み立て、公開境界で型付きSafetyProgramへ変換する。明示v2は型付きIRを保持し、同じ木を解析・更新してSafetyProgramを返す。型付きIRへの移行自体を性能改善の根拠にはしない。既存TS frontendは独立した差分比較対象として残す。

`compile_with_options(&module, &config, CompileOptions { irVersion: IrVersion::V1 })`は明示v1を選ぶ。`compile`の既定はv1のままで、通常のv1 loweringを維持する。別のEXCEPTへ渡す自由な`@`の既知の捕捉不一致は、v2を要求する拒否へ変更する。`CompileOptions { irVersion: IrVersion::V2 }`は[安全性IR v2](../../docs/safety-ir-v2.md)を明示選択する。到達するexpression定義を字句宣言順とphase / membership用途 / target signature順の定義表へ一度だけlowerし、call / parameter / capture参照を生成する。引数とcaptureはcallerのlexical frameを持つ遅延引数としてrtlcが要求時ごとに評価し、値をmemoizeしない。binder slotは定義frameごとに0から始め、引数・capture・caller binderのためframeを必要とする非再帰actionも共有し、frame不要の零引数呼出しはinlineを維持する。明示v2はRECURSIVEの自己・相互再帰expressionを実行し、v1拒否からv2へ自動再試行しない。

frontendの式nestingとbound variableは40、checkerの定義内式深さは48、値深さは64、TS decoderは128のまま。定義間call経路長を定義内の式nestingへ加算せず、v2の動的な評価は別に入れ子128までとする。call entryと遅延引数・captureの要求はrtlcのwork予算へ各1課金し、枯渇はbounded / workになる。生成IRが既存のcompiled expression decoder深さ128を超える場合は両frontendが同じ型で拒否する。共有は評価仕事の指数増大の除去や、全TLA+との互換性を意味しない。

v2のfrontendは、同じ種類の`And`・`Or`が直接の項になっている連鎖と、`ELSE`が直接`IF`である連鎖を、明示的なloopでたどり、連鎖全体を1段の入れ子として数える。式とmembershipの位置では連言・選言は左のspineだけをたどり、右結合の入れ子は段ごとに数える。action・SPECIFICATIONのparts・ENABLEDの位置では左右どちらの入れ子もたどる。Label・LET・演算子の呼び出しは連鎖を切り、時間論理の連言・選言は対象外とする。出力IRは同じ意味の綴り（平たい連言・選言、OTHER付きCASE）と同じで、v1の数え方とIRは変えない。IRの深さは連鎖の長さに比例するので、checkerの定義内式48は緩めない。最上位のinvariantに置いたとき、平たい連言・選言は48項、IF連鎖とCASEは47枝までが受理で、超える入力（54枝のIF連鎖を含む）は検査時に`Safety expression nesting exceeds 48`で拒否する。連鎖を呼び出しの中に重ねた式はv2の動的評価128で拒否する。Rustのparserは、最も外側まで括弧で囲んだ62段以上の連鎖を`TLA parsing: Expression nesting exceeds 64`で拒否し、TS frontendはparseする。

root moduleの`EXTENDS Apalache`がある場合、`:=`を同モジュールの定義どおり等号として扱う。`EXTENDS Variants`ではApalache 0.63.0の`Variant`、`VariantTag`、`VariantGetUnsafe`を扱い、tag取得はdomainが一つの文字列keyである関数に限定する。他のVariants operatorと無制限CHOOSEは未対応。`EXTENDS TLC`は受理するが、TLCの全operatorを実装するものではない。

CASEは式・actionともOTHERを省略でき、ソース順で最初の真のarmを選ぶ。OTHERがなく全条件が偽なら、TLC v1.7.4 (`5a47802`) と同様に評価時に未一致エラーとソース位置を返す。未評価の分岐内ではエラーにしない。

defining module が直接 `EXTENDS Apalache` を指定するときだけ、`ApaFoldSet(Op, v, S)` と `ApaFoldSeqLeft(Op, v, seq)` を扱う。`Op` は呼び出し位置で可視の top-level / LET 定義のoperator名またはLAMBDAで、零 arity の引数をちょうど2つ持つ必要がある。累積値が第1引数、要素が第2引数。foldのframeでbound slotを2つ使い、v1では呼び出し位置でbodyを展開する。v2では同じslotをactualとして共有expression定義をcallする。Rust frontendはLAMBDAとoperator parameterを静的に特殊化する。再帰callbackはv2を要求し、他のApalache operatorはチェックリストの範囲に従う。`EXTENDS Apalache` がなければ fold 名も通常の名前解決に従う。展開深さ・bound variable の上限は変更しない。

sequence は先頭から処理し、文字列・record・関数を含む任意の要素種を扱う。集合 fold は TLC v1.7.4 (`5a47802`) の順序に合わせ、整数、Boolean、tuple / sequence、集合、整数 key の関数と、それらの再帰的な組み合わせを扱う。整数は数値順、Boolean は FALSE / TRUE、tuple / sequence と集合は要素数の後に再帰的な辞書順、関数は domain 要素数の後に各 key / value を交互に比較する。集合要素内の全整数 function key の最大値と最小値の差が `i32::MAX` を超える場合は拒否する。TLC の interval / 明示 domain 間の32-bit減算で順序が反転するためで、IR は元の domain 表現を区別しない。primitive 整数や関数の値にこの幅制限は課さない。domain が `1..n` の関数は既存の値表現どおり sequence として扱う。文字列・record をどの深さでも含む集合は、IR に TLC の文字列・field 名の intern ID がないため、要素が 2 個以上で結果が順序に依存しうる fold を、それぞれ理由を明示した評価時エラーで拒否する。順序に依存しないと IR から分かる fold は受理する。要素が 1 個以下の集合（種類を問わない）、body が要素を読まない fold、body が累積値に `\union` で集合を足す fold（足す集合と IF の条件は累積値を読まない）である。 累積値は、足す `\union` の直接の被演算子でなければならない。`acc \union ({elem} \union {"x"})` は受理し、`(acc \union {elem}) \union {"x"}` は受理しない。要素が2個以上で文字列・record を含む集合は、要素の中に比べられない組（field の種類の食い違いなど）があれば、fold を受理する前に従来の拒否を返す。累積値に足す fold の body が評価エラーになる場合と、結果の要素に比較できない組（整数と型付き model value など）がある場合は、TLC が最初に報告するエラーが順序で決まるため、従来の拒否を返す。置き換えない例外は、要素が 1 個の集合（body のエラーをそのまま返す）、`Unsupported TLA input` で始まる別の拒否、予算切れ（`budget`・`evaluation-budget`）である。非整数 key のその他の関数は従来の `Unsupported TLA input: set fold order for strings, records or functions` を維持する。要素が 2 個以上の集合 fold の model value は未対応で、異なる値種の比較が必要な集合も拒否する。[pinned TLC ログ](../../fixtures/tlc-apalache-folds/README.md)を対照にしており、Apalache 全体との互換性を表さない。

Rust経路はmodel value、有限CHOOSE、整数の\div / % / ^、CONSTRAINT / ACTION_CONSTRAINT、PROPERTY内の定数有限domain上の時間論理量化にも対応する。制約は探索候補を捨てる前に境界のinvariant/propertyを検査する。無解のCHOOSE、ゼロ除算、0^0、overflow、typed model valueの不正比較は評価エラー。空domainの時間論理量化は比較元TLCのtautology診断も含めて検査する。

[固定入力](tests/finite-support.json)をnative/Wasm/TLCへ渡す。既存TS frontendの対応範囲は別で、追加機能はcompileTlaSource / checkTlaSourceから呼ぶ。CriterionではASTからのcompileと、生成済みIRのcheckを別々に測る。parser、IPC、process起動は計測区間へ含めない。

```sh
just compiler-test
just compiler-criterion finite-compiler evals/runs/my-compiler-bench
just tlc-finite-eval evals/runs/my-tlc-check /path/to/java /path/to/tla2tools.jar
```

生成的UNCHANGEDのtargetだけをcallerの字句環境から変数/tupleへ静的解決し、解決済みtarget signatureごとに式定義を共有する。通常のactual/captureはcallerのlexical frameを保持し、membership用途も伝播する。action/temporal外殻も共有callまたはanonymous frameでbinder・old・parameter・captureへの遅延参照を保持する。LET内の自由な`@`はTLC対照に従う。詳細と検証範囲は[Safety IR v2](../../docs/safety-ir-v2.md)を参照。

明示v2のaction/temporal helperは共有callまたはanonymous frameでcallerの遅延引数を保持する。binder・EXCEPT oldと要求時のprimeを分離し、入口と引数要求へ各1 workを課金する。LET内の自由な`@`はTLC対照に従う。Rust frontendの明示v2では再帰する代入actionを共有定義とcallへ変換する。引数・capture・caller binderのためframeが必要な非再帰actionとtemporal helperも共有する。零引数でframe不要の呼出しはinlineを維持する。[呼出しと制限](../../docs/recursive-actions.md)を参照。

## モジュールと設定置換

Rustでは`compile_modules(&root, &config, &modules)`へ解析済みModuleの集合を渡す。`compile`は単一module用の既存APIで、外部moduleを自動取得しない。TSの`compileTlaSource` / `checkTlaSource`とnativeの`compile-tla` / `check-tla`には、追加の`modules: { ModuleName: source }`を渡せる。キーとMODULE名が異なる場合は拒否する。modulesを省略した既存要求も受理する。

EXTENDSと引数付きnamed/unnamed INSTANCEを解決する。WITH右辺は呼び出し側で解決し、束縛名を区別して展開する。変数の置換先は呼び出し側のstate variable名に限定する。LOCALは他moduleへ公開せず、内部の定義から参照できる。LET内INSTANCE、nested module、名前・LAMBDAのoperator引数を解決する。全moduleのSANY検査やlevel検査の代替ではない。

TLC設定の`N <- Bound`、同arityの`Op <- Replacement`を扱う。module別の定義に対する`Op <- [Module] Replacement`、`Value = [Module] 2`、`Op(1) = [Module] 2`も扱う。設定の置換先はrootで解決する。operator宣言そのものへの引数付きテーブル設定は未対応。比較元TLCがoperator値をstateに含めてエラーにする対照を保存する。不正な設定の診断phaseと文言、存在しないmoduleへの値設定の無視まで同一にはしない。

IR v1/v2の両方でmodule連結と設定置換を扱う。引数付きINSTANCE、LET内INSTANCE、nested moduleの宣言位置のscopeを保持する。変数置換はstate variableのaliasへ制限する。

追加moduleは256、importの深さは40、展開後のsymbolは4096まで。ファイルシステムやネットワークを使わず、欠落・循環・名前の衝突はエラーにする。[固定corpus](tests/modules-support.json)と[TLC比較の計画](../../evals/plans/0047-module-linking.md)で対応範囲を検査する。

反例のaction名は公開名、instanceの修飾名、LETの元のoperator名へ戻す。内部の束縛識別子を実装との照合に使う必要はない。

ASTの書き換え深さは64段まで。ModuleをRustから直接渡した場合も、未使用の定義を含めて上限を検査する。

```sh
just module-eval evals/runs/my-module-check /path/to/java /path/to/tla2tools.jar
just module-replay evals/runs/my-module-check evals/runs/my-module-replay /path/to/java /path/to/tla2tools.jar
```

## 標準演算と量化された公平性

定義元が`EXTENDS Sequences`を指定すると`SelectSeq(sequence, Predicate)`を扱う。Predicateは可視の1引数operator名またはLAMBDA。`EXTENDS TLC`の`SortSeq(sequence, Before)`は2引数operator名またはLAMBDAを取る。LETの捕捉とlexical scopeを保持し、再帰callbackはv2を要求する。空列ではcallbackを評価せず、SortSeqのsingletonも比較しない。SelectSeqは順序・重複を保つ。SortSeqは新要素を既存列へ挿入し、比較が真の間、直前の要素を移動する。等順位の順序は比較operatorに従う。

`Assert(condition, message)`は両方の引数を評価し、conditionがTRUE以外なら評価エラーを返す。messageは文字列に限定しない。RandomElement、TLCGet、TLCSetは未対応。

`SPECIFICATION`内の`\A p \in Processes: WF_vars(Step(p))`と対応するexists/SFを扱う。domainは定数の有限集合。tuple・入れ子・空domain・定数引数のwrapperに対応し、state依存domainや量化したInit/Nextの一般的な分解は拒否する。空existsによりfairnessが偽になる入力は、比較元TLCと同じくtautologyのモデルエラーとして検査する。fair behaviorの存在の一般的な証明ではない。

[固定入力](tests/operators-support.json)はnative/Wasm/TLCとChromiumで検査する。反例は元のSPECIFICATIONを保持してTLCで再生する。`just operators-eval RUN JAVA JAR`と`just operators-replay SAVED RUN JAVA JAR`で実行・再現する。

SYMMETRYはIR v1でflat moduleのSymm == Permutations(Nodes)に限定してlowerする。TLC importと、直接割り当てた2〜64個の異なるuntyped model valueの集合を必要とする。明示v2でSYMMETRYを指定するとコンパイル時に拒否する。rtlcの状態形状・safety限定の条件に従い、宣言のequivarianceは利用者の前提。[条件と制約](../../docs/symmetry.md)を参照。

引数なしLET定義の自由な`@`は宣言側の環境を保持し、引数あり定義の`@`は適用側の環境に従う。Rustのv2では、別のEXCEPTへ入る引数なし定義を明示した遅延引数へ変換する。この組合せはv1と旧TS frontendで理由を示して拒否する。

source level検査、遅延する有限関数とtemporal template共有の範囲は[frontendの意味検査](../../docs/frontend-semantics.md)を参照。全SANYの名前・型・proof検査とは区別する。

明示v2のtuple target解析と共有IRの変数読取り展開は、それぞれ524,288ノードまで。空tupleの複製も数え、超過はcompile時のprotocol拒否となる。既存のnesting 40とbranch 256 stepsは維持し、実行時のworkとは別に扱う。

v2の時間論理IRは、式深さとは別に構造の深さを128までとする。flatな連言・選言から生成した木も、frameのcapture整理に入る前に検査する。超過は`compiled temporal nesting exceeds 128`で拒否する。

Quintの実生成物6モデルは既定/v1/v2のnative/Wasm回帰へ接続する。`quint_generated` Criterion benchは同じ生成物で解析・compile・prepared check・全体を分ける。crate内の[bench入力](tests/quint-generated.json)は生成TLA+を変更せず保持し、リポジトリの回帰で原本とTLCオラクルへ照合する。[実行手順](../../docs/quint-generated.md)を参照。
