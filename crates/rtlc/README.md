# rtlc

`SafetyProgram::prepare(self)`は所有する`PreparedSafetyProgram`を返し、静的検証とData索引を繰り返すcheckへ再利用する。ASSUME・定数invariant・値の評価はcheckで行い、Arena・visited・work・coverageは毎回新規となる。[再利用の契約](../../docs/prepared-safety.md)を参照。

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

有限TLA+ fragmentのモデル検査ライブラリ。コンパイル済みIRから状態を生成し、BFS、invariant/deadlock、temporal property、WF/SFを検査する。通常BFSのsafety反例は最短で、temporalのlassoとopt-in PORでは元のgraphでの最短反例を保証しない。nativeと`wasm32-unknown-unknown`で同じ実装を使う。

実行コードは`idealis-core`へ依存しない。RustのTLA+経路ではrtlaが構文解析、rtla-compilerがIR生成を担当する。既存のTypeScript frontendも比較対象として残す。[対応範囲と制約](../../docs/rust-safety.md)を参照。TLC CLIやTLA+全体の互換性を示すcrateではない。

整数の\div / % / ^、有限CHOOSE、state/action constraint、model value、定数有限domain上の時間論理量化を扱う。探索制約で除外した候補もinvariantと対応するpropertyを検査する。一般のSYMMETRY、VIEW、ディスク退避、checkpoint、simulation/DFID、式単位のcoverageは未対応。各項目は[チェックリスト](SUPPORT.md)で管理する。

select-seq / sort-seqのIRは、1引数の選択predicateと2引数の比較bodyを束縛slotで評価する。sortはTLCの挿入sortと同じ順序で比較し、常に真・偽の比較も扱う。反復・比較はwork予算を消費する。assertはconditionとmessageを順に評価し、TRUE以外ならmessage付きの評価エラーを返す。対応するTLA+ソースの制約はrtla-compilerが定める。

model valueのwire表現は`{"type":"model-value","value":"p_1"}`。文字列の`"p_1"`とは異なる値で、反例にもtagを保持する。型名はTLCと同じく名前の先頭1文字とunderscoreで表す。集合foldでmodel valueの順序を使う入力は拒否する。

時間論理の展開では、256式以下のpoolの訪問済み式を固定長bit集合で保持する。分岐のcloneでHashSetのhash・確保を繰り返す費用を減らす。257式以上のpoolはHashSetを使い、分岐ごとに全pool分のbitmapを確保しない。式の展開順とworkのtickは同じ。

## アクションのcoverage

`program.check_with_coverage(bounds)`は`SafetyCoverageReport { result, coverage }`を返す。resultは通常のcheckと同じ判定・反例・work。coverage.initialとcoverage.nextはIR branch順の配列で、同名のbranchも統合しない。attemptsは評価を始めた回数、enabledは完成した候補を1つ以上生成したattempt数。generatedは重複・constraint除外を含む完成候補数、distinctは保存した新規state数で、最初に発見したbranchへ帰属する。

各行のworkはbranchの評価と受理時のconstraint/property検査を含む。事前検査や時間論理等のworkはunattributedWorkへ残す。行のgenerated/distinctの和はresultのgenerated/statesに一致し、行のworkの和とunattributedWorkはresult.workに一致する。boundedや違反では途中までの計数になる。評価エラーは通常と同じErrを返す。

これは逐次BFSのIR branch計数。式・source location単位や並列のcoverage、TLCの全cost modelは未対応。TLCのInit行は重複や棄却候補を含むcounterを出し、活性反例の再構築も行counterを増やすため、出力の全数値の一致は保証しない。workは時間やTLCの式costではない。通常checkではgeneric recorderで計測処理を除く。

## Rustから使う

ローカルcheckoutでは`rtlc = { path = "../idealis/crates/rtlc" }`を依存に追加する。JSON IRを読む場合は`serde_json`も使う。

```rust
use rtlc::{SafetyBounds, SafetyProgram};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let json = std::fs::read_to_string("program.json")?;
    let program: SafetyProgram = serde_json::from_str(&json)?;
    let result = program.check(SafetyBounds {
        max_depth: 100,
        max_states: 100_000,
        max_work: 10_000_000,
        max_temporal_nodes: None,
    })?;
    println!("{} states, {}", result.states, result.status);
    Ok(())
}
```

`rtlc::ir`に`Expression`、`Branch`、`Step`、`Predicate`、`TemporalFormula`などの公開契約を置く。JSONを経由しない[型付きcounterの例](examples/counter.rs)もある。Arena、探索queue、worker、intern tableはprivate。`Expression`・`Branch`・`TemporalFormula`・`Value`は深さによらず有界なstackで破棄するため`Drop`を実装し、これらの値から非Copyのfieldをpatternでmoveできない。参照で照合し、`Vec`・`String`のfieldは`std::mem::take`、`Default`を持たない`Expression`・`TemporalFormula`・`Value`のfieldは`Expression::Old`・`Value::Null`などの葉との`std::mem::replace`で取り出す。

既存の`modelt.safety-program` v1を既定の入力形式として維持する。v2ではexpression定義表とcall / parameter / captureを受理し、callerのlexical frameを持つ引数を要求時に毎回評価する。call entryと引数評価へ各1 workを課金する。明示recursiveのexpression定義は受理し、強連結成分の依存を固定点で検証する。v2はaction定義とStepのcall、temporal定義とTemporalFormulaのcallも受理し、Init/Nextのphase・call kind・arityを検査する。定義ID・capture順とwire契約は[IR v2](../../docs/safety-ir-v2.md)を参照。両frontendは明示`irVersion: 2`でこのIRを生成する。型所属ではv2のidentity range map-setを列挙せず検査する。`complete`、`bounded`、`empty`、違反と評価エラーは区別する。`bounded`を安全性の証明として扱わない。`Value`と`CoreError`はidealisと共有するwire契約で、idealis-coreからも同じ型を再exportする。汎用Valueのvariantが存在することをTLA検査側の対応保証にはしない。

nativeの`check_parallel(bounds, workers)`は1〜256 workerの上限を受け取る。軽いscalarや狭いfrontierは逐次実行する。action/temporal propertyとfairnessを含む探索も逐次へfallbackする。Wasmは逐次の`check`を使う。

`check_with_options(bounds, SafetyOptions)`でworker上限とopt-inの遷移削減を選べる。`symmetry_actions`は限定SYMMETRYの同じnode行の代表展開、`partial_order`は他の操作・invariantが参照しない単調なBool flag操作の順序削減。既定は両方false。SYMMETRYとPORは併用できない。[対応条件と反例の制限](../../docs/reductions.md)を参照。

固定TLCとの照合には、temporal helperへ定数の量化式を渡す既知の乖離が1入力ある。TLC一致として数えず、保存したログと実装の意味結果を別に検査する。[IR v2の乖離と制限](../../docs/safety-ir-v2.md)を参照。

`check_incremental(bounds, &mut cache, options)`は、`ExpansionOptions { enabled: true, build_digest, max_entries }`で状態ごとの完了したNext評価を再利用する。optionsの既定値は無効で、通常の`check`へ委譲しcacheを参照・更新しない。Init・property・constraintsは毎回検査し、visitedやpropertyの合否を引き継がない。portable値、全Next/definitionのidentityとSHA-256を持つsnapshotを保存・mergeできる。状態keyとsuccessor列には1展開64 KiBの保守的な変換予算を設け、超過時は保存せず通常のstreaming探索を続ける。この経路は逐次探索。[契約と証明範囲](../../docs/safety-expansions.md)を参照。

## 検証と配布

```sh
just rtlc-test
just rtlc-example
just criterion-test
just package
```

`rtlc-test`は全target、fmt/clippyとCriterion smokeを実行する。Criterion14本とfixtureはこのcrateに置く。通常の時間計測は`just criterion-save <stage> <新規runパス>`。

`rtlc`はRustのrlibとしてWasmにもcompileできる。ブラウザ向けのJSONL/host-importなしWasm ABIはidealis-coreが提供する。`just package`はrtlc/idealis-coreの両archiveを作り、展開したcrateだけで単独consumerを検査する。registryへのpublishは行わない。

## 関数値の内部表現

関数・record・sequenceは正規化したdomainを共有し、値を別に保持する。64要素以下の全Boolと全Int(0/1)は別のbit表現を使う。BooleanのFALSEと整数0は区別する。その他の値と65要素以上はScalar列へ戻す。EXCEPTは元の関数を変えず、domain外の更新と入れ子の扱いを維持する。

workerで作ったdomainはkeyとpayloadのRefをrootへ移す。rootのdomainはimmutableに共有する。domainの所属metadataはTLA+値の等値性に含めない。hash衝突時はdomainと値を完全比較し、公開JSON、IR、workと反例の形式は変えない。[0056の比較](../../evals/iterations/0056-compact-functions.md)を参照。

packed関数と完全に一致する有限rangeでは、domainを確認してから、要素数と同じ論理workを一括計上する。異なる型・rangeとbudget付きworkerは元の要素順に検査する。通常のmembershipも値だけのowned iteratorを使い、key/value列の構築を避ける。

nativeのforkでは、rootのdomainが16個以下なら、64 key以下のdomainにworker専用wrapperを使う。候補更新ごとに全workerが同じArcカウンタへ書く費用を避け、merge時にrootのdomainへ戻す。対象外の入力は通常の共有へ戻し、対応する仕様の範囲は変えない。

汎用Scalar列のEXCEPTは、domainとpayloadを借用したviewで更新後の値を検索する。既存の値があればpayloadのcloneとdomain参照の取得を省く。新規の値だけを確保し、同じhashを登録に使う。packed値とpackedへ復帰する更新は既存の経路を使う。hash衝突時はdomainとpayloadを完全比較する。[0057の比較](../../evals/iterations/0057-probe-updates.md)を参照。

packed関数の内部hashはcached domain hash・payload・Bool/Intのsaltをfmix64で混合する。MurmurHash3のpublic domain実装を参照し、公式C++のgolden vectorへ照合する。衝突時はdomain/payloadを完全比較し、hash形式を公開APIへ含めない。[0058の比較](../../evals/iterations/0058-packed-hash.md)を参照。

Data式はprogram内のアドレスからslotを作り、各Arenaで初めて評価が成功したScalarを再利用する。値全体のhashを繰り返さず、未評価分岐や不正値を先に評価しない。失敗はcacheせず、未登録の式は従来の評価を使う。workerのlocal Refはmergeで補正し、別のcheckへcacheを持ち越さない。[0059の比較](../../evals/iterations/0059-prepared-execution.md)を参照。

## 限定したSYMMETRY

全nodeをdomainに持つInt/Bool関数、flatなInt/Bool recordの関数、node部分集合とglobal Int/Boolの状態を、同じnodeの値の行で同一視する。複数関数の相関と具体的な反例を保ち、全N!置換を列挙しない。SYMMETRY宣言がInit/Next/invariantを保つことは利用者の前提。自動的な対称性の証明ではない。

単一の2〜64個のuntyped model value集合とsafety検査に限定する。typed値・node参照やnested payload・temporal/fairness・constraints・initial/action property・coverageとIR v2は対象外。nativeではfrontierが64状態以上なら並列展開し、それ未満は逐次へ戻す。JSONのsymmetryはoptionalで、Rustのstruct literalにはsymmetry: Noneを追加する。[設定例と適用条件](../../docs/symmetry.md)を参照。
v2のmembership descriptorはcall/parameter/captureと条件分岐を通って要求時ごとに解釈する。通常の値とv1の列挙は維持する。

明示v2のaction/temporal helperは共有callまたはanonymous frameでcallerの遅延引数を保持する。binder・EXCEPT oldと要求時のprimeを分離し、入口と引数要求へ各1 workを課金する。sourceの対応範囲はcompilerが判断する。再帰expressionとaction定義は明示v2で扱う。Rust frontendが生成する再帰actionの範囲は[呼出しと制限](../../docs/recursive-actions.md)を参照。

v2の関数選択とDOMAINはalias/actualを通る遅延関数を扱う。完全なScalarを要する位置では有限domainを実体化する。[観測位置と制限](../../docs/frontend-semantics.md)を参照。
