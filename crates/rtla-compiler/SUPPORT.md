# rtla-compilerの対応チェックリスト

チェック済みはrtlaのASTをrtlcの有限検査IRへ変換する範囲。未チェックは未対応または対象外で、実装予定を約束しない。構文解析と探索は他crateが担当する。

## 対応済み

- [x] `analyze_modules` で実行用設定なしのモジュール束に既存 linker と任意の level 検査を適用する。単独解析で proof・nested module は拒否する。直接 EXTENDS Sequences の Len 等と同名のトップレベル宣言には、衝突元と定義位置を示す。

- [x] 共有IRの統合でも既存v1のSYMMETRY設定を保持する。明示v2のSYMMETRYはlowering前に理由を示して拒否する。

- [x] `CompileOptions.irVersion`でv1を既定・明示選択し、明示v2ではexpression定義を共有するSafetyProgramを返す。通常のv1 loweringと省略時の選択を保持し、v1拒否から自動再試行しない。別のEXCEPTへ渡す自由な`@`はv2を要求する。
- [x] v2の定義IDを字句宣言順とphase / membership用途 / target signature順、captureをlexical bindingの宣言順で決める。frame-localなbinder slotとcall / parameter / capture参照を生成する。
- [x] 定義graphの循環・level依存解析を明示stackで処理し、LETスコープで宣言ASTを参照共有する。5000定義のcompile-only対照を持ち、実行成功とは区別する。
- [x] 生成的UNCHANGEDとSPECIFICATION subscriptの変数・tuple別名をparameter / capture経由も含め解決する。ENABLED内にも同じ対象解決を使う。v2では要求されるformal/capture targetをsource依存metadataで解析し、signature別に式定義を共有する。foldの専用frame付き呼出しも共有を維持する。
- [x] v2の引数・captureをcaller lexical frame付きの遅延引数としてrtlcへ渡す。値のmemoizationは行わず、call entryと要求時の課金・予算判定はrtlcが担当する。

- [x] 名前解決、定数・operatorとLETの展開、lexical scopeの保持。
- [x] 既存語彙のcustom記号operatorを通常の定義として解決する。prefix `-.`、infix、postfix `^+`/`^*`/`^#`とnonfix呼出しをIR v1/v2で扱う。
- [x] builtin・対応する標準moduleからimportした記号とのbinding衝突を拒否する。未使用定義とLET/formal宣言も検査し、SANY全体の意味解析とは区別する。
- [x] 明示したModule集合のcompile_modulesで非再帰EXTENDSと引数なしnamed/unnamed INSTANCE、WITH、LOCALを解決する（IR v1/v2）。diamond import、定義元のscope、独立したinstanceを保持する。
- [x] v2で連結と設定上書きを要さない入力は、追加moduleを添えてもbundleの検証が通れば、`compile_modules_with_options`が`compile_with_options`と同じrootの変換を行い、変換に成功した場合は同じIRを返す。連結の検証による拒否はv1と同じmessageになる。
- [x] 反例のaction名には公開名・instanceの修飾名・LETの元のoperator名を使い、内部の束縛識別子を出さない。
- [x] signed 32-bit整数、Bool、文字列、有限集合・関数・record・sequenceの対応式とactionの変換。
- [x] 有限量化、tuple pattern、EXCEPT、全変数の代入・選択・UNCHANGED、列挙できるENABLED。
- [x] IFとCASE。最初の真のarmを選び、OTHERがなく未一致なら評価時エラーにする。未評価の分岐ではエラーにしない。
- [x] INIT/NEXTまたは対応形のSPECIFICATION、定数、INVARIANT/PROPERTY、CHECK_DEADLOCK、定数ASSUME。
- [x] 対応するstate/action/temporal propertyとWF/SFをIRへ変換する。
- [x] 定義元moduleのEXTENDS ApalacheでApaFoldSet / ApaFoldSeqLeft。可視の2引数operatorを使う。再帰callbackは明示v2に限る。
- [x] EXTENDS ApalacheでSetAsFun。`SetAsFun(S)`は`DOMAIN`が`{p[1] : p \in S}`で各keyを`p[2]`へ写す関数で、`Map()`、`Map(k -> v, ...)`、`setToMap()`の生成物を受理する。同じkeyに異なる値の組があれば、CHOOSEの選択順を保証できないため評価時エラー`SetAsFun: a key has more than one value`で拒否する。
- [x] 同じimport下の`:=`をApalache定義の等号として扱う。
- [x] EXTENDS VariantsでApalache 0.63.0のVariant / VariantTag / VariantGetUnsafe。tag取得はsingletonの文字列domainに限定する。
- [x] EXTENDS TLCのimport自体を受理する。任意のTLC operatorを受理する意味ではない。
- [x] 展開・変数・分岐・step等の上限と未対応入力を拒否し、型付きSafetyProgramを返す。

## TLC互換の追加対応

- [x] CONSTANTで与えるmodel valueとtyped model valueを専用のdata値へ変換する。文字列とは区別する。
- [x] 整数の\div・%・^を専用IRへ変換する。評価時の意味論はrtlcが担当する。
- [x] 有限domainに限定したCHOOSEとtuple pattern。無制限CHOOSEは拒否する。
- [x] CONSTRAINT / ACTION_CONSTRAINTを名前付きpredicateへ変換する。
- [x] PROPERTY内の時間論理forall/exists。domainは定数の有限集合に限定し、通常のstate量化と区別する。
- [x] flat moduleのゼロ引数定義Symm == Permutations(Nodes)とTLC import、CONSTANTへ直接与えた2〜64個の異なるuntyped model value集合をoptionalなsymmetry.modelValuesへlowerする。宣言のequivarianceを自動証明せず、実行時の状態形状と組合せはrtlcが制限する。
- [ ] 一般のSYMMETRY設定、複数群・任意の置換・typed model value・module bundle/INSTANCEとの併用。VIEWの設定とlowering。
- [ ] ALIASによる反例表示、POSTCONDITION、POSSIBLE、PERIODIC、RL_REWARD。
- [x] CONSTANTのoperator置換（<-）と同arityのoperator宣言への置換。module別の定義への値・operator・引数付きテーブル設定。
- [ ] operator宣言そのものへの引数付きテーブル設定。比較元TLCがoperator値をstateへ格納してエラーにする入力を未対応として拒否する。
- [x] IR v1/v2でEXTENDS SequencesのSelectSeqとEXTENDS TLCのSortSeqをlowerする。callbackは可視のoperator名またはLAMBDAを静的に特殊化し、SelectSeqは1引数、SortSeqは2引数。定義元scope、LET捕捉とprimeを保持する。
- [x] IR v1/v2でEXTENDS TLCのAssertをlowerする。conditionとmessageの両方を評価し、TRUE以外はmessage付きの評価エラーにする。
- [x] SPECIFICATION内の定数有限domain上のforall/exists fairness。tuple・入れ子・空domainと定数引数のwrapperを扱う。state依存domainと量化したInit/Nextの一般的な分解は対象外。
- [ ] TLCのRandomElement、TLCGet、TLCSet。
- [ ] 最小整数リテラル-2147483648のTLC診断互換。Rust frontendの既存の例外受理と比較元TLCには差がある。計算式-2147483647 - 1は対応する。

## frontendの追加対応と残る制約

- [x] IR v2のSelectSeq / SortSeq / Assertのsource lowering。
- [x] 明示v2のRECURSIVEによる自己・相互再帰、有限domainの暗黙再帰関数。scalar値・guard・temporal atom・callbackで実行する。
- [x] 明示v2でRECURSIVEの自己・相互再帰するInit/Nextの代入actionを共有action定義とStep callへ変換する。LET捕捉、INSTANCE、高階特殊化、ENABLEDと継続を扱う。引数・capture・caller binderのためframeが必要な非再帰actionも共有する。零引数でframe不要の呼出しはinlineを維持する。recursive自身のformalをprime/UNCHANGED対象にする入力はSANYに従って拒否する。
- [x] 明示v2でframeを必要とする非再帰actionとtemporal helperをbody kind別の共有定義へ変換する。PROPERTYのatom/always分類とcall入口・actual要求のworkを保つ。非再帰actionの仮想展開にも4096経路/256 stepsを適用する。
- [x] source levelの制約を未使用定義も含め検査する。prime・UNCHANGED・ENABLED、temporal/action operands、formal/capture/LET・再帰の制約を固定点で伝播し、引数の受理上限と結果への依存を区別する。全SANYの名前・型・proof検査ではない。
- [x] 明示v2で再帰関数をalias・operator actual経由で渡し、DOMAINと選択keyを遅延評価する。宣言時state・binder・oldのsnapshotはrtlcが保持する。等値・状態保存など完全な値を要する位置では有限domainを実体化する。
- [ ] 一般のlazy関数値。EXCEPTのtarget、fold、membershipなど通常のScalar評価を使う観測位置の完全なTLC遅延評価互換。
- [x] v2でもmodule連結、設定operator置換・定数表・定義上書きをlowerする。
- [x] v2のfrontendは、同じ種類の連言・選言が直接の項になっている連鎖と、`ELSE`が直接`IF`である連鎖を、明示的なloopでたどり、連鎖全体を1段の入れ子として数える。式とmembershipの位置では連言・選言は左のspineだけ、action・SPECIFICATIONのparts・ENABLEDの位置では左右どちらの入れ子もたどる。右結合の入れ子（式の位置）、`IF`のTHEN側、Label・LET・演算子の呼び出しをまたぐ入れ子、時間論理の連言・選言は段ごとに数えたままにする。出力IRは同じ意味の綴り（平たい連言・選言、OTHER付きCASE）と同じで、v1の数え方とIRは変えない。
- [ ] checker（rtlc）も連鎖を1段と数えるが、IRの深さは連鎖の長さに比例する。最上位のinvariantでは平たい連言・選言128項、CASE127枝までを受理し、超える入力はfrontendの出力検査128で拒否する。JSONの要求とTSのdecoderを通る経路では、decoderの128でより短い長さで拒否されうる。Rustのparserは最も外側まで括弧で囲んだ62段以上の連鎖を`TLA parsing: Expression nesting exceeds 64`で拒否し、TS frontendはparseする。
- [ ] v2は、tuple別名の連鎖`V0 == x`、`Vn == <<V(n-1), V(n-1)>>`に対する`UNCHANGED Vn`を、n=7（展開後のleafが128個）以上で検査時に`Safety evaluation nesting exceeds 128`で拒否する。v1はn=8（leafが256個）まで受理する。測定した現在の限界で、原因は調べていない。

- [x] 引数付きINSTANCE、LET内INSTANCE、nested moduleの宣言時scope、名前・LAMBDAによるWITH operator置換。
- [x] 高階operatorをarityと字句scopeに従って静的に特殊化する。LAMBDAと一般selectorを解決する。
- [x] labelのbound parameter、重複、EXCEPT境界、RECURSIVE宣言の位置・arityを未使用定義も含め検査する。
- [ ] operatorをstateへ格納する一般の値、任意の式への変数置換、全SANYの意味・level検査。
- [ ] 無限集合の列挙、stateに依存するdomainでの時間論理量化、temporal variable quantification（\AA / \EE）。
- [ ] 無制限CHOOSE、他のApalache/Variants operator（ApaFoldSet / ApaFoldSeqLeft / SetAsFun / `:=`を除く）。
- [ ] 全モジュールのSANY検査、未使用定義の一般的な名前・型検査とproofの正しさの判断。
- [ ] 一般のTLA+からSMTへの変換、Quint言語の直接解釈、Quint Connect APIやITFのimport。

## 制約と根拠

- [x] v2のUNCHANGED・ENABLED・SPECIFICATION subscriptのtuple別名展開は、v1と同じ入れ子の数え方で40を超える入力を`expression or operator nesting exceeds 40`で拒否し、node上限内の固定したtuple chainでv1の受理境界と照合し、「非対応・対象外」に記したUNCHANGEDの差を保持する。UNCHANGED・ENABLEDの展開はleafが後段の枝の上限と同じ256個を超える入力を`Safety branch exceeds 256 steps`で拒否する。
- [x] frontendの式nesting / bound variable 40、checkerの定義内式48 / 値64、TS decoder128とv2の動的な評価入れ子128を緩めない。call経路長と定義内式のnestingを区別する。
- [x] 集合foldの値種と順序の制約を守る。文字列・recordや一部の関数を拒否する範囲と、順序に依存しないため受理する範囲は[README](README.md)に従う。
- [x] 未対応・型エラー・予算切れを検査成功へ読み替えない。探索予算と結果はrtlcの契約。
- [x] source bundleの追加moduleは256、importの深さは40、展開後のsymbolは4096まで。欠落・循環・衝突を拒否し、ファイルシステムやネットワークを読まない。
- [x] Moduleを直接渡す場合もASTの書き換え深さは64段まで。未使用の定義でも過度に深いASTを拒否する。
- [x] 不正な設定の拒否phase・診断や、存在しないmoduleへの値設定をTLCが無視する場合は同一診断を保証しない。未対応拒否とTLCの評価エラー・無視を比較記録で分ける。

[実装](src/lib.rs)、[回帰](tests/compiler.rs)、[追加対応の回帰](tests/finite_support.rs)、[比較入力](tests/finite-support.json)、[標準演算と公平性の回帰](tests/operators.rs)、[その比較入力](tests/operators-support.json)、[Quint生成物の12設定](../../docs/quint-generated.md)、[既存frontendとの比較](../../evals/iterations/0029-rtla-compiler.md)を参照。
[operator共有の回帰](tests/operator_sharing.rs)も参照。
[連鎖の入れ子の回帰](tests/nesting_chains.rs)も参照。
[記号operatorの回帰](tests/symbolic.rs)、[binding検査](src/symbols.rs)と[構文・TLC比較](../../evals/iterations/0069-symbolic-operators.md)も参照。高階operatorは静的に特殊化し、operatorをstateへ格納する一般の値とは区別する。

[operator-sharing cohort](../../tests/tlc-differential/operator-sharing-test.ts)は保存TLCログ、TS/Rust構造、native / Wasm完全応答、線形成長と小予算のbounded / workを検査する。固定した有限fragmentと入力での根拠であり、全TLA+互換や指数的な評価仕事の除去、一般的な速度改善は主張しない。

生成的UNCHANGEDはcallerの字句環境で変数/tuple targetを解決する。通常式の引数・captureを外殻へ代入せず、membership用途を伝播する。action/temporal引数は共有callまたはanonymous frameでcallerのlexical bindingを保持する。LET内の自由な`@`はTLC対照に従って扱う。引数の`@`を呼出し先のEXCEPTへ渡す場合はv2を要求する。

- [x] 明示v2の非共有anonymous expression/action/temporal frameを扱い、callerのbinder・old・引数を遅延参照する。入口と引数要求へ各1 workを課金し、v1・既存上限を維持する。LET内の自由な`@`はTLC対照に従って扱う。引数の`@`を呼出し先のEXCEPTへ渡す場合はv2を要求する。

再帰経路の動的評価は128段まで。超過はbounded/evaluation、work枯渇はbounded/workで、成功と区別する。暗黙再帰関数の直接key適用とDOMAINは別にlowerする。関数値全体の構築は有限・eagerであり、任意のaliasを通じた完全なlazy関数値は未対応。

別のEXCEPTへ入る引数なしLETの自由な`@`は、宣言側の環境をv2の遅延引数で保持する。引数あり定義の適用側oldとは区別し、この組合せのv1は拒否する。

[追加対応と独立比較](../../evals/iterations/0070-tla-coverage.md)を参照。構文受理・有限実行・証明の検証を区別する。

[意味検査・遅延関数・共有templateの範囲](../../docs/frontend-semantics.md)、[level回帰](tests/levels.rs)、[lazy回帰](tests/lazy_functions.rs)と[共有回帰](tests/templates.rs)を参照。

- [x] v2のtuple target解決と共有IRの変数読取り展開は、それぞれ524,288ノードを上限とする。空tupleを繰り返す場合も数え、超過を`target expansion exceeds 524288 nodes`で拒否する。既存のnesting 40・leaf 256と別のcompile時上限で、実行時のworkやメモリの厳密な上限ではない。

- [x] anonymous frameのactual/captureは、格納とcloneの前にframeの1段を含むcompiled depth 128を検査する。過度に深い入力はprotocolで拒否する。

- [x] frame_actualsはargumentsとcapturesの各Expressionを初期depth 1で検査し、全件通った後にVecをcloneする。128を超える式はprotocolで拒否する。

- [x] frameのbodyもcapture compaction前にcompiled depthを検査する。

- [x] v2は時間論理IRの構造的な深さを式深さと別に128で制限する。flatな連言・選言からできる木もframeのcapture compaction前に検査し、超過を`compiled temporal nesting exceeds 128`で拒否する。v1のcompile時受理は変えない。

- [x] 固定したQuint 0.33.1 / Apalache 0.63.0の6生成物・12設定を、省略v1・明示v1・明示v2でnative/Wasmに照合する。生成元・生成TLA+とTLCオラクルの一致を回帰で検査する。Quint全体への互換性ではない。
- [x] 同じ6生成物の解析・v1/v2のcompile・prepared check・parse+compile+checkをCriterionで分離する。計42測定の各compile/check経路はTLCの正常判定・状態数・生成数を検査してから測定する。CIでは時間の閾値を設けずsmoke実行する。
