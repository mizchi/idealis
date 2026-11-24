# Safety IR v2

`modelt.safety-program`は省略・明示`irVersion: 1`でv1を生成する。明示`irVersion: 2`ではTS/Rustの両frontendが到達するexpression定義を共有するv2を生成し、checkerが検証・実行する。v1拒否から自動再試行しない。request envelopeとWasm ABIのversionは1のまま。

Rust frontendのv2はmodule連結と設定置換に対応する。引数付きINSTANCE、LET内INSTANCE、nested moduleも有限fragmentへlowerする。TS frontendの受理範囲は独立しており、自動でRustへ切り替えない。

SYMMETRYはv1だけで扱う。v2 frontendは宣言を拒否し、手書きv2 IRのsymmetryも実行器が拒否する。限定PORはv2のframe/callを独立と推測せず、解析できない操作を通常探索へ戻す。Rust frontendはSelectSeq / SortSeq / Assertをv1/v2へlowerする。

## Wire契約

全count/indexは`WIRE_INDEX_MAX = 4294967295`以下の0以上の整数に統一する。対象はDefinitionのid/parameterCount/captureCount/localScope、expression・action・temporal frameのparameterCount/captureCount/localScope、expression/Step/TemporalFormulaのcall.definition、parameter/capture/bound/readのslot、Bindingとbind stepのslot/tupleSize、assign/choose stepのslot、fold.accumulatorの27フィールドである。配列長とindexにも同じ範囲を適用する。TS decoderとRustのwire deserialize・直接構築したIRの検証は同じ範囲を使い、Rustはusize変換前に検査する。nativeとWasmで別の上限を使わない。範囲内でも実在ID・arity・slot・localScopeの制約は別に検査する。未使用DefinitionのparameterCount/captureCountは上限値を受理し、上限+1をprotocolで拒否する。bounds/resultの既存uint32契約は維持する。

v2はv1のprogramフィールドに必須の`definitions`配列を追加する。v1ではこのフィールドと新しい式を拒否する。未知のフィールド・versionも拒否する。v2 decoderはENABLEDの省略可能な`subscript: null`を保存し、native/Wasmの完全応答を一致させる。TS/Rustの正規化構造比較では従来どおりこのnullを省略できる。

```json
{
  "id": 0,
  "parameterCount": 1,
  "captureCount": 0,
  "bodyKind": "expression",
  "localScope": 0,
  "body": { "op": "parameter", "slot": 0 }
}
```

`id`は配列indexに一致する0始まりの連続整数。`parameterCount`と`captureCount`は各参照領域の大きさ、`localScope`は定義frame内のbinder slotの容量（0〜40）。binderは導入時のlocals数と同じslotを使い、calleeのlocalsは空から始める。parameter / captureはbinderと別領域である。bodyKindはexpression、action、temporal。actionのbodyはBranch配列で、`{kind:"call", definition, arguments, captures}`のStepから呼ぶ。temporalのbodyはTemporalFormulaで、`{kind:"call", definition, arguments, captures}`のformulaから呼ぶ。3種類のcallのbody kindを交換できない。Rust frontendの再帰action生成は[呼出しと制限](recursive-actions.md)を参照。

```json
{
  "op": "call",
  "definition": 0,
  "arguments": [
    { "op": "literal", "value": { "type": "bool", "value": true } }
  ],
  "captures": []
}
```

`arguments`と`captures`は必須で、定義の各countと一致する。`parameter`と`capture`はそれぞれ`slot`だけを持つ式。全定義・全actualを未選択の分岐や未使用引数も含めて検証し、評価エラーになる式を検証中に実行しない。未知ID、recursive未指定の循環、参照・arity・slot・phaseの不整合はprotocolエラーになる。

## 両frontendの決定順

定義の同一性は名前ではなく字句上の宣言位置で決める。top-level宣言をソース順に走査し、各宣言内のLETを出現順に深さ優先で走査する。到達するexpression定義をこの順へ絞り込み、同じ宣言のspecializationを下記の固定順に並べて0からIDを振る。呼び出しの発見順やhash mapの反復順でIDを決めない。同名のshadowingは別定義である。

captureは自由なlexical bindingの集合を、束縛の宣言順へ並べる。外側のscopeから内側へ、同じscopeのformal parameterは宣言順、binderは導入順とする。shadowされた同名bindingを名前でまとめない。EXCEPTの外側`@`もそのscopeで導入されたbindingとして明示captureする。parameter thunkのcaptureは値へ強制せずに保持する。

定義entryのkeyは宣言位置・phase・用途・解決済みtarget signature・body kind（expression、action、temporalの順）とする。phaseは`constant` / `initial` / `state` / `action`、用途は`value` / `membership-type`の順に固定する。各宣言の到達するkeyだけをこの順に並べ、callは対応するentryのIDを参照する。initial内の変数読取りの表現と、state / actionでの読取りを同一bodyに混ぜない。membership-typeは列挙せずに型集合を検査する用途で、引数・captureも要求時に同じ用途で解釈できる。生成位置が要求するformal/captureを依存順で解析し、call siteのcaller/宣言環境をたどって変数slotまたはtupleへ解決する。signatureは要求bindingの初出順にdescriptorを並べ、slotをtupleより先、slot番号とtuple要素を数値・構造の辞書順で比較する。tupleの順序と入れ子は保持し、同じsignatureだけを共有する。keyをwireの追加フィールドにせず、definition IDの決定規則として両frontendへ適用する。TS/Rustの共有actionには同じ規則を使う。temporal templateの自動共有はRust frontendが行う。operator-sharing cohortではtemporal callを検証してanonymous frameへ展開した後、定義表・call・branch・step構造を比較する。

## 評価と検証

callはactualの式とcallerのimmutableなlexical frame（locals、parameter / captureのthunk、EXCEPTのold）を持つ新しいframeを作る。parameter / captureは読むたびに式を評価し、値cacheを持たない。current / next / primeはlexical frameへ固定せず、読んだ時点の動的contextを使う。定義本体の外側oldは直接読めず、明示captureが必要。ENABLEDのfresh next列挙と短絡順は従来どおり。

v2のmembership descriptorでは、単一のrange bindingとそのslotだけを返す`map-set`を整数区間として検査する。`tupleSize`を持たないidentity形だけを対象とし、区間を列挙せず両端を評価する。空区間、model value、整数以外の診断は従来のrange所属と同じ順に処理する。v2のIF/CASEで選ばれたmembership結果へも用途を保ち、条件と選択枝を毎回評価する。未選択枝を評価せず、通常の値としての`map-set`とv1の評価は列挙を維持する。

call entryと引数・captureの要求時にそれぞれ`Arena::consume(1)`へ課金する。直接評価`Id(Id(TRUE))`は4 workで、maxWork 3 / 4 / 5は枯渇 / 成功 / 成功となる。探索の既存費用は別に加算する。枯渇はbounded / reason:workで、上限を超えない。membership descriptorもcallを毎回通し、cacheでこの課金を省略しない。

定義graphを先に検証し、明示再帰の強連結成分を固定点で解析する。phase別の受理条件・formalへの依存とconstant / context-free metadataをactualとcaptureへ合成する。callを葉として判定しない。frontend nesting 40、checkerの定義内expression深さ48、値深さ64、TS decoder深さ128は変更しない。call graphの経路長を定義内の式深さとして加算しない。v2の動的な式・membership・branch・binder評価は別に入れ子128までとし、再帰経路の超過はbounded/evaluation、通常の不正な深さはprotocolエラーにする。値やcall結果のcacheではなく、検証contextごとのmetadata解析結果だけを再利用する。

v2のcheckerは、`and`の`left`が直接`and`である連鎖（`or`も同じ）と、`if`の`alternative`が直接`if`である連鎖を、段ごとに再帰せず明示的なloopでたどり、連鎖全体を1段の入れ子として数える。式の評価、遅延関数、membershipのIF、構造の検証、metadataの解析の5か所で同じ規則を使う。連鎖の先頭で1段進め、各項・各条件・各枝・最後の`alternative`はその深さで評価・検証する。連言は最も左の項から評価して偽で止まり、選言は真で止まり、IF連鎖は条件を先頭から評価して最初に真になった枝だけを評価する。評価順・短絡・最初に報告するエラー・workは段ごとに数えたときと同じで、IR・wire・v1の数え方は変えない。`right`側と`consequent`側の入れ子、`implies`、時間論理の連言・選言、actionのbranchは段ごとに数える。連鎖を1段と数えてもIRの深さは長さに比例するため、v2の検証は式の木の根からのIRの深さが128を超える入力を`Safety IR expression nesting exceeds 128`で拒否する。frontendの出力検査とJSON・TSのdecoderも128なので、この拒否はRustのAPIで直接組んだIRにだけ現れる。最上位のinvariantに置いたとき、Rustの経路では平たい連言・選言は128項、CASEは127枝、括弧つきの連鎖とIF連鎖はparserの上限の手前の61段まで受理し、それより長い入力はfrontendの出力検査128かparser 64で拒否する。

v2のfrontendは、同じ種類の`And`（または`Or`）が直接の項になっている連鎖と、`IF`の`ELSE`が直接`IF`である連鎖を、段ごとに再帰せず明示的なloopでたどり、連鎖全体を1段の入れ子として数える。連鎖の各項と各枝は、連鎖の先頭で1段進めたcontextでlowerする。式とmembershipの位置では、連言・選言は左のspineだけをたどって今までと同じ左結合の二項を作り、IF連鎖は今までと同じ入れ子の`if`を作る。右結合の入れ子と`IF`のTHEN側の入れ子は段ごとに数える。action・SPECIFICATIONのparts・ENABLEDの位置では、連言・選言の左右どちらの入れ子もたどり、branchの組み立て順と`limited`の検査位置は入れ子のときと同じにする。Label・LET・演算子の呼び出しは連鎖を切り、時間論理の連言・選言は対象外とする。部分式をlowerする順序（最初に報告するエラー）は変えない。出力IRは同じ意味の綴り（平たい連言・選言、OTHER付きCASE）と同じで、v1の数え方とIRは変えない。連鎖の長さに比例するのはIRの深さなので、checkerの定義内式48は緩めない。checkerも同じ連鎖を1段と数える（評価と検証の節）。IRの深さが128を超える入力はfrontendの出力検査が拒否する。Rustのparserは、最も外側まで括弧で囲んだ62段以上の連鎖を`TLA parsing: Expression nesting exceeds 64`で拒否し、TS frontendはparseする。この差は変更前からあり、この変更では揃えない。

## 検証範囲

rtlc unit testsは不正IR、frame-local slot、直接評価のwork境界を検査する。operator-sharing cohortではTS/Rustが生成するv2を保存済みTLCログと照合し、chain、binder、lexical capture、未使用 / 使用引数、prime、ENABLEDを検査する。両frontendの正規化後構造、native / Wasmの完全応答、定義数・call edge数・serialized bytesの線形増加も検査する。両枝を評価するchainは小さいwork予算でbounded / workになることを要求し、指数的な評価仕事の除去とは数えない。

UNCHANGED・ENABLED・SPECIFICATION subscriptのtuple別名展開は、v1と同じ入れ子の数え方で40を超える入力を拒否し、受理する集合を、次に述べるUNCHANGEDの限界を除いてv1に一致させる。UNCHANGED・ENABLEDの展開はleafが後段の枝の上限と同じ256個を超える入力も拒否する。v2はtuple別名の連鎖`Vn == <<V(n-1), V(n-1)>>`に対する`UNCHANGED Vn`を、n=7（展開後のleafが128個）以上で検査時に`Safety evaluation nesting exceeds 128`で拒否し、v1はn=8（leafが256個）まで受理する。これは測定した現在のv2の限界で、原因は調べていない。

v1の深いinlineとbinder混線は型付き拒否として別に保持し、既存239入力は省略・明示v1のserialize digestで確認する。0070では追加のCHOOSE対照8件と生成actionの旧対照について、別のEXCEPTへ渡す自由な`@`をv2要求の拒否へ変更する。既存239入力のdigestは保持する。手書きv2 IRの対照はfrontend生成の照合と区別する。これらは固定した有限fragmentと入力での証拠であり、全TLA+互換、全モデルの受理、一般的な速度改善を意味しない。

## 深い定義と対象位置

定義graphの循環検査とlevel依存解析は明示stackによる依存順で行う。RustのLETスコープは宣言ASTを参照共有し、スコープ作成ごとに全定義の本体を複製しない。5000定義の前向きchainはinvariant / guard / prime / membershipのcompile-only対照でTS/native/Wasmの構造を照合する。これは深いchainの実行成功を保証しない。

生成的UNCHANGEDとSPECIFICATIONの変数subscriptでは、別名・parameter・captureを変数または変数tupleへ解決する。ENABLEDのbranch生成でも内部の生成位置だけを解決する。通常式のUNCHANGEDと`[A]_e` / `<<A>>_e`のsubscriptは値比較を維持する。target resolverはsource actualとcaller/宣言環境を追い、定数・binder・`@`・計算式を変数targetとして要求された場合はprotocolで拒否する。通常のparameter/capture参照とcaller frameを保ち、同じformalのtarget以外の参照もparameterのままにする。対象の解決は評価結果のmemoizationではない。[固定TLCの処理](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/impl/Tool.java#L1268-L1279)と新規3入力のログを対照とする。

CASEのarm列は従来どおりif/elseへlowerするため、浅いsourceでも生成IRがdecoder上限を超える場合がある。v2では既存のcompiled expression decoder深さ128を超える出力を両frontendが同じprotocol拒否として返す。JSON decoderの深さ128とcheckerの式48も維持するため、frontendが受理してもcheckerで拒否される入力は残る。169 arm CASEのchecker JSON拒否はv1でも再現する既存制限であり、対応範囲や上限は広げない。

生成的外殻全体へのactual IR代入は行わない。式定義はtarget signatureごとに共有し、EXCEPTのcaller oldとcallee old、caller binderとcallee binderを分ける。membership位置へ達するformal/captureの要求も定義graphで伝播し、actualを両frontendでmembership用途へlowerする。IF/CASE/LETのmembership結果にも用途を保持する。

frameが必要な非再帰action外殻はTS/Rustとも共有callへ変換する。Rustはtemporal外殻も共有し、TSのtemporalはanonymous frameを使う。零引数でframe不要の呼出しはinlineを維持する。生成targetだけをsourceと環境から解決し、通常actualはcaller lexical frame付きthunkとして渡す。binder・old・parameter・captureをcalleeのlocalsへ貼り付けない。Rust frontendの明示v2は再帰する生成actionも共有action定義とStep callへ変換する。
foldのoperator呼出しはaccumulator/element用の明示actualと専用frameを持つため、通常のshared callを維持する。ENABLEDとbinderを持つfoldをv1/v2で照合する。

## 非共有anonymous frame

明示v2だけで、expressionは`{op: "frame", arguments, captures, parameterCount, captureCount, localScope, body}`、action stepは`{kind: "frame", arguments, captures, parameterCount, captureCount, localScope, branches}`、temporal formulaは`{kind: "frame", arguments, captures, parameterCount, captureCount, localScope, formula}`を使う。いずれも全フィールド必須で未知フィールドを拒否する。countsは配列長に一致し、localScopeは40以下。非再帰のanonymous frame自体はdefinition tableへ登録しない。再帰actionは別の共有定義を使う。

arguments/capturesはcaller scopeで検証し、bodyは空のlocals・独立parameter/capture領域・oldなしで検証する。callerのbinder/formal/oldを参照する式は引数または明示captureとして保持する。未使用actualと未選択branchも検証し、評価は要求時だけ行う。frame内部のshared callもdefinition graphとphase依存解析へ含める。

action frameの内側branchが終わると、内側のnext候補を維持し、callerのlexical contextで外側の残りstepsを実行する。backtrackで代入を復元し、callee binder/frame/oldを外へ漏らさない。ENABLEDにも同じ実行境界を使う。temporal frameは所有されたlexical snapshotを保持し、atomの評価時の状態を使う。prime/current/nextやmembership用途をframe作成時の値へ固定しない。

frame入口とparameter/captureの要求へ各1 workを課金し、既存step・探索・列挙の費用も維持する。値や結果はmemoizeしない。anonymous expressionの`Id(Id(TRUE))`も入口・要求の合計4 workで、maxWork 3/4/5は枯渇/成功/成功となる。frameの埋込みbodyも深さとbranch/stepの上限へ集計し、既存制限を迂回しない。

Rust frontendはLET内の自由な`@`をTLCとの実行対照で検査する。actual自身の`@`はcaller frameへ保持する。異なるEXCEPTへ渡すactualの`@`はIR v2を要求する。

v2の値依存ENABLED対照で使うBoolean等価`<=>`は、両辺の否定を等号で比較する形へlowerする。両辺を左から一度ずつ評価し、Boolean型を要求する。v1の受理範囲・serialize結果は変更しない。

`Temporal(p) == [](p)`へ定数の量化式を渡す`sharing-action-frame-temporal`は`[]TRUE`を意味するが、固定TLCはtemporal違反を報告する。この1入力だけを[記録済みTLC乖離](../fixtures/tlc-operator-sharing/divergences.json)とし、保存ログ・exit13・初期x=0からstutteringの反例・状態数1/生成数2を固定して検査する。v2は両frontendでcomplete・状態数1/生成数2・深さ0・work11を要求し、正規化構造とnative/Wasm完全応答も照合する。直接の`[]TRUE`・`[](forall)`は従来のTLC success goldenと同じ意味結果を要求する（workは各5/7）。固定revisionのtableauが`<>FALSE`を畳まず整合・promise充足と扱うという説明は[固定ソース](https://github.com/tlaplus/tlaplus/blob/5a47802b5c391f59ecdd44117981f4ff8c0656ba/tlatools/org.lamport.tlatools/src/tlc2/tool/liveness/TBPar.java#L243-L265)の読取による推論であり、内部tableauを計装した実行確認や他revisionへの保証ではない。この乖離をTLC一致へ数えず、状態依存actualの`sharing-action-frame-temporal-state`も従来どおりTLC未対応（exit255）として一致・乖離のどちらにも数えない。anonymous temporal frameの手書きIR検査、意味結果の検査、frontend生成入力のTLC一致は区別する。

step 3e-bの追加5仕様測定では、Aのv2が両frontendで受理され、deadlock・325状態・深さ9・work97078となった。Aのv2は31741 bytes・20定義、v1は61610 bytesで、保存済みv1と同じ違反である。Bはcompiled decoder128、Cは未対応operator、D/Eはfrontend nesting40でv2拒否が残る。Cの診断表現にはfrontend差も残る。B Rust v1の300秒timeoutを型付き拒否へ読み替えず、全仕様への適用や一般的な速度改善とはしない。詳細と測定区間・限界は[iteration](../evals/iterations/0048-operator-sharing.md)へ記録する。

## 有限CHOOSEとsource解析

明示`irVersion: 2`ではTS/Rustの両frontendが有限domainの`CHOOSE n \in S: P`とtuple patternを`choose`式へ変換する。domainはcallerの環境で、predicateは選択binderを導入した環境でlowerする。actual内のcaller binderとEXCEPTのoldも遅延frameで保持する。空domain・成立する要素がない場合・predicateやtupleの型が不正な場合は、compile拒否ではなく評価時エラーになる。未選択枝や未使用actualでは評価しない。省略・明示v1のTS受理範囲は変えない。選択順の互換性は保存済みTLC対照の範囲に限る。

sourceのfree-old、target要求、membership要求、generative分類は、frontendごとの型付き子列挙とrole・scope・forwarding規則を使う。IF/CASEの条件は値位置で、結果枝だけへincoming用途を伝える。`\in`と`\notin`の右辺はmembership用途を開始し、生成的な所属代入は従来どおり`\in`だけを扱う。CHOOSEのdomain/bodyへincoming用途を伝えず、その内部の所属やENABLEDは各位置の規則で読む。LET定義の自由なold検査は未使用定義と関数headのdomainも読み、EXCEPTのreplacementだけがoldを束縛する。

v2のIR解析・変換もfrontendごとの型付き子列挙を使う。levelは定義bodyが要求するactual/captureだけを合成し、capture収集とlocalScopeはnested frameのbody境界を越えない。DAG・call収集/再番号付け・prime/phase・compiled depthは未使用actualと全frame bodyも読む。Rustはpending定義のbodyを未確定のままdeserializeせず、lower完了後の定義・一時式・lexical環境・programを型付きIRで保持する。更新adapterと読み取りは同じ列挙から導出する。TSも同じdescriptorのpathでimmutableに更新する。v1は既存のloweringと走査を使う。

action展開はbranch.stepsとaction frame.branchesのedgeだけを数え、actual/captureや値式内部のENABLEDは加えない。各stepとframe wrapperを1 stepとし、frame bodyの最大step数を加算する。各branchのframe bodyの経路数は乗算し、branch間は加算する。4096経路/256 stepsは受理し、超過は既存の診断で拒否する。非再帰action callにも同じ仮想展開の上限を適用する。再帰callは従来どおり展開しない。PROPERTYのatom/always分類はtemporal frameまたはcallのbodyを読み、actual/captureを保持する。分類後に到達しないtemporal定義は除去する。sourceとIRの検査成功を全programのparityや全TLA+互換の証明とは扱わない。

## 再帰expression

Definitionのoptionalな`recursive: true`は自己・相互再帰を明示する。省略とfalseは従来どおり循環を拒否する。強連結成分の全定義にフラグを要求し、phase・parameter・captureの依存を固定点で検査する。未評価分岐でも不正なphaseやarityを許可しない。

再帰経路の動的評価は128段までで、超過は`bounded / evaluation`となる。work枯渇は`bounded / work`。制限を超えた再帰をcompleteやemptyにしない。値のmemoizationは行わない。Rust frontendは明示v2の再帰actionを生成する。

引数なしLET定義の自由な`@`は宣言側の環境を保持し、引数あり定義の`@`は適用側の環境に従う。Rustのv2では、別のEXCEPTへ入る引数なし定義を明示した遅延引数へ変換する。この組合せはv1と旧TS frontendで理由を示して拒否する。

## temporal共有と関数観測

temporal callのactual/captureはcaller scopeで検証し、bodyは独立したparameter/capture領域で検証する。temporal定義のphaseはNextに限り、`recursive: true`でもtemporalの循環を拒否する。式の共有呼出しと同じ128段の動的評価上限を使う。共有化は評価結果のcacheではなく、同じbodyをIRに重複して格納しない変更である。

v2の関数選択とDOMAINは内部の遅延関数を扱う。wireの値tagや公開Scalarは追加しない。宣言時snapshotとcaller thunkは異なる境界で、関数が所有するstate/locals/old/primeからbodyを評価する。共有化前後ではworkを維持する。別の変更である関数選択の遅延化は、一時的な全関数の構築を省くためworkが減る場合がある。[対応する観測位置](frontend-semantics.md)を参照。

## tuple展開のcompile上限

v2のtarget解決と共有式からの変数読取りには、各524,288ノードの上限を適用する。深さが浅い場合や空tupleの複製も数え、超過は`target expansion exceeds 524288 nodes`で拒否する。実行時のworkや厳密なRSS上限とは別の制約。既存のdepth 40、action leaf 256と正常なtuple chain 16の対照を維持する。

Rust frontendはanonymous frameのactual/captureを格納・cloneする前に、frame入口の1段を含むcompiled depth 128を検査する。TS frontendの省略/明示v1はbaseでの受理範囲を維持する。compiled depth 128の新たな拒否はv2だけへ適用する。

両frontendはv2の時間論理IRの構造的な深さも128までに制限する。式の深さとは別に、連言・選言を含む全Temporal nodeを数える。frameのcapture整理前に超過を`compiled temporal nesting exceeds 128`で拒否し、v1のcompile時受理範囲は変えない。

資源検証のRSSはLinux/macOSの`/usr/bin/time`が子プロセス終了時に返す最大RSSを使う。`ps`のサンプリングは早期停止用に分け、OS値が欠落・ゼロなら検査成功にしない。これは評価harnessの依存で、pureなWasmコアにOS APIを追加しない。
