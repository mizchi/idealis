# rtlcの対応チェックリスト

チェック済みはコンパイル済みの有限検査IRに対する機能。TLA+ソースの受理とは区別する。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] IRのwire構造はSafetyProgramのformat/version/name/variablesと遷移・predicate・定義を持つ。maxDepth/maxStates/maxWorkはu32、maxTemporalNodesはoptionalなu32。SafetyResultはstatus/reason/states/generated/depth/work/violationを持つ。実行時の意味と拒否は検証器・探索部で扱う。

- [x] SafetyProgram::prepareが不変のprogram・静的検証metadata・Data索引を所有し、繰り返すcheckとcoverageへ再利用する。各探索のArena・visited・workは独立し、prepareで値やASSUMEを評価しない。[再利用の契約](../../docs/prepared-safety.md)を参照。
- [x] 固定TLCとtemporal helperの既知の1乖離を、TLC一致と区別して保存・検査する。[IR v2の範囲](../../docs/safety-ir-v2.md)を参照し、全TLA+互換とは扱わない。

- [x] v1のSYMMETRY・代表展開・並列展開・限定PORを共有IRと共存させる。v2のSYMMETRYは拒否し、frame/callで独立性を確認できないPORは通常探索へ戻す。
- [x] v2のsequence callbackでもframe-local slotと共有callを検証する。SelectSeq / SortSeq / Assertの深い子の破棄も有界に扱う。source loweringの対応とは区別する。
- [x] prepared定数はv2の共有定義・frameも走査する。深いaction frameの準備は明示stackで行い、上限超過をprotocolで拒否する。

- [x] SafetyProgramからsuccessorを生成し、BFSで状態を重複排除する。hash衝突時にも状態全体を比較する。具体グラフの reachable_graph は制約を適用した完全探索だけを返す。初期・後続状態の未代入時は branch 名と未代入変数を診断する。Value の Display と SafetyViolation::format_trace で変数名付きの反例を表示できる。
- [x] invariant・deadlock・初期state property・action property・定数ASSUMEの検査とsafety反例の復元。
- [x] 対応するtemporal formula、stuttering、WF/SFとlasso反例。safetyと異なり最短lassoは保証しない。
- [x] signed 32-bit整数、Bool、文字列、有限集合・有限関数。recordとsequenceも有限関数として等価性を判定する。
- [x] IRの算術・集合・関数・record・sequence操作、有限量化、EXCEPTと@、foldの評価。
- [x] 未一致CASEの評価時エラーと、singletonの文字列domainに限定したvariant-tag。
- [x] select-seq / sort-seqのcallback IRとassert。選択は順序・重複を保ち、sortはTLCと同じ挿入順で新要素と直前要素を比較する。callbackのslotを検査し、反復・比較でworkを消費する。assertはmessageも評価する。
- [x] maxDepth/maxStates/maxWorkとtemporal積グラフの予算を扱う。
- [x] safety IR v1を維持し、明示v2のexpression定義表とcall / parameter / captureを検証・評価する。未指定の循環とID、arity、capture、frame内slot、phase違反を拒否する。明示recursiveの強連結成分はphase summaryの固定点で検証する。
- [x] v2の引数はcallerのlexical frameを保持し、要求時のcurrent / next / primeで毎回評価する。値cacheを持たず、call entryと引数評価に各1 workを課金し、枯渇をbounded/workにする。
- [x] recursiveフラグ付きv2の自己・相互再帰expression定義。
- [x] v2の共有action定義とStep call。expression/action/temporalのkind、arity、capture、phaseと再帰SCCを検証し、代入・分岐の巻き戻しとcallerの継続を実行する。再帰評価の128段超過はbounded/evaluation、work枯渇はbounded/workにする。
- [x] callのactual/captureは一つのimmutableな親frameを共有する。値や評価結果はcacheせず、要求時のworkとcurrent/next/primeを維持する。ソース生成の受理範囲とは区別する。
- [x] nativeのcheck_parallelは1〜256 worker上限を受け取り、適したsafety frontierを並列展開する。
- [x] 軽いscalar・狭いfrontier・action/temporal property・fairnessは逐次へfallbackする。Wasmは逐次check。
- [x] Rust rlibとしてnative/Wasmへビルドでき、idealis-coreやparser、外部solverには依存しない。
- [x] 関数・record・sequenceのimmutable domainを共有し、64要素以下の全Boolまたは全Int(0/1)を別tagのbit列に保持する。混在・範囲外・65要素以上はScalar列へ戻す。公開Value/IRとcanonical identity、workを維持する。
- [x] domainと関数のhash衝突を完全比較し、worker新規domainのkey/payload Refをrootへremapする。root所属domainはimmutableに共有し、所属metadataを等値性へ含めない。
- [x] Int/Ref domainのlookupは候補添字のkey一致を検証し、一致しない場合は線形検索へ戻す。疎なdomainと意味順がRef割り当て順と異なるdomain、欠落key、更新後の元の不変性を保つ。
- [x] packed関数の型と完全に一致する有限rangeのmembershipでは、domain確認後に同じ要素数の論理workを一括計上する。workerのbudget付きoverlayは既存の要素順へ戻す。通常の関数検査も値だけのowned iteratorを使う。
- [x] native forkはroot domainが16個以下なら、64 key以下のdomainにworker専用wrapperを持ち、参照カウンタを分離する。merge後はcanonical rootへ戻し、対象外は通常のArc共有へ戻す。

- [x] 汎用Scalar列の関数更新は、コピー前のborrowed viewでbase/localの既存値を検索する。hash衝突はdomain/payloadを完全比較し、新規の値だけを確保する。packed値とpackedへの復帰、workと深さの検査を維持する。

- [x] packed関数の内部hashはdomain/payload/型saltをfmix64で混合する。公式C++の7出力、固定4,096値の分布、Bool/Intの衝突で完全比較とcanonical identityを検査する。汎用関数のhashと公開Value/IRは維持する。

- [x] immutableなprogramのData式をslotへ登録し、Arenaごとに初回成功後のScalarをcacheする。未評価分岐の値を先に評価せず、失敗・状態依存式をcacheしない。未登録の式は従来の評価へ戻り、workerのlocal Refはmergeでremapする。
- [x] Data式のslot索引では内部address専用hashと完全なkey比較を使う。ユーザー値・集合・関数・探索状態のhashと等価性、lazy評価・work課金は変更しない。
- [x] 連続するbindの後続が読まない束縛を持つ場合、読む束縛の値の組ごとに後続を一度だけ評価し、同じ組では記録したsuccessor列を同じ順序でvisitorへ渡し直す。状態・生成数・反例・探索順を維持する。再生では継続の入口とsuccessorごとに1 workを課金し、workは変更前を超えない。計画はPreparedProgramがprogramの持つbranchにだけ作り、複製したbranchとENABLEDを含む後続は従来の評価へ戻す。記録は1回の展開の中だけで使い、継続とsuccessorの件数に上限を持ち、再生のない並びでは記録をやめる。

- [x] 任意のcheck_incrementalで、同じbuild・Next・全definition・変数順とportable状態について、完了したsuccessor列だけを再利用する。Init・property・constraints・deadlock・temporal/fairnessは毎回検査し、visitedや過去の合否を引き継がない。保持状態数の上限、snapshotのSHA-256検査と同じnamespaceのmerge・競合拒否を扱う。この経路は逐次探索で、分散BFSやcheckpoint再開ではない。
- [x] 状態keyとsuccessor列のportable変換は1展開64 KiBの保守的な予算で制限する。共有値のサイズを複製前に計算し、超過時や保存容量不足時は通常のstreaming探索を続け、不完全な展開を保存しない。
- [x] ExpansionOptions.enabledは既定false。無効なら通常のcheckへ委譲し、cacheを参照・更新せず、今回のhit/miss/writeを0とする。再利用には明示trueとbuild digest・保持上限を渡す。

## TLC互換の追加対応

- [x] model valueを文字列と区別して保持・比較し、typed model valueの不正な等価比較を評価エラーにする。portable値のtagはmodel-value。
- [x] 整数のfloor division、正の除数によるmodulo、非負指数のpower。ゼロ除算・0^0・i32 overflowは評価エラー。
- [x] 有限domainの式CHOOSE。正規化した順序の最初の一致を選び、tuple pattern、短絡と無解時エラーを扱う。Step::Chooseによる状態の候補代入とは別。
- [x] state/action constraintで探索を制限する。除外する境界候補もinvariantと対応するpropertyを検査し、除外をdeadlockとは扱わない。completeは制約後のグラフに対する結果。
- [x] 時間論理のforall/existsを定数の有限domainで展開する。tuple、空domain、入れ子を扱い、展開もwork予算を消費する。
- [x] temporal展開の訪問済み式は256式以下で固定長bit集合に保持し、それより大きいpoolはHashSetへ戻す。todo/next/pendingの順序とworkのtickは変えない。
- [x] 単一の2〜64個のuntyped model value群に対する限定SYMMETRY。全群をdomainに持つ関数のInt/Boolまたはflat recordと、node部分集合を扱う。recordのfield名・Int/Bool型を関数内で揃える。同じnodeの関数値・集合所属を同じ行として完全比較する。個別の群要素・順序依存式と未対応shapeは拒否する。
- [x] 同一視用keyと具体状態を分離し、元の具体状態から探索・traceを作る。node行を連続配置し、検査済みの先頭recordと同じScalarだけschema再構築を省く。key構築と代表選択の論理workは同じ値でも維持する。宣言がInit/Next/invariantを保つことは利用者の前提で、gateをequivariance証明とは扱わない。
- [x] SafetyOptionsのsymmetry_actionsはopt-in。先頭に単一node bindingを持つbranchの同じ行を代表だけ展開する。Init・複数binding・tuple binding・全群以外のdomainは通常展開。generated/workは削減後の計数。
- [x] nativeのsymmetryは64状態以上のfrontierを並列展開する。rootへのRef移送後に逐次にkeyを照合し、代表展開とも併用する。狭いfrontierとWasmは逐次へ戻す。
- [x] SafetyOptionsのpartial_orderはopt-inで逐次実行する。invariantと他branchが参照せず、他branchが戻せないBool flagのFALSE→TRUE操作だけを先に実行する。依存・未知の式・不正な状態型では通常展開へ戻す。temporal/fairness・constraint・initial/action propertyでは通常探索、SYMMETRYとの併用は拒否する。元のグラフの最短反例は保証しない。
- [ ] 一般のSYMMETRY、複数群、typed model value、node参照やrecord内のnested payload、temporal/fairness・constraint・initial/action property・coverageとの併用。
- [ ] 任意の独立性解析や循環に対応する一般のPOR。現在の限定PORの条件外への適用。
- [ ] VIEWによる状態の同一視。
- [ ] ディスクへの状態・queueの退避とcheckpoint/resume。
- [ ] seed付きのIR simulation、DFID探索。
- [x] check_with_coverageの逐次action coverage。initial/nextのIR branch順を保ち、attempts、enabled、generated、distinct、受理時の検査を含むworkとunattributedWorkを返す。resultとwork予算は通常checkと一致する。
- [ ] 式・source locationごとのcoverage、TLCの全cost model、並列coverage。
- [ ] TLC CLIの全オプションと反例出力形式。ALIASはcompiler/host側の未対応項目。

## 移植性・追加保証と対象外

- [ ] TLA+の構文解析・名前解決・IRへのコンパイル、TLC CLI全体の互換性。
- [ ] 無限状態の完全探索、任意のTLA+構文やSMT理論。TLC互換性の不足とは別に扱う。
- [ ] Wasm内部の探索マルチスレッド、temporal/fairness探索の内部並列化。
- [ ] 型別命令VM、訪問済み照合やstate受理の内部並列化。0059の試作と0061の命令IDによる直接実行は後退があり不採用。
- [ ] liveness反例の最短性、fairnessを満たすbehaviorの存在の別証明。
- [ ] このcrate単体のブラウザJSON ABI。Wasm境界はidealis-coreが提供する。

## 制約と根拠

- [x] complete / bounded / empty / 各違反 / 評価エラーを区別する。boundedやemptyを安全の証明にしない。
- [x] v2の型所属はidentity range map-setを列挙せず判定する。通常値のmap-setとv1の評価は列挙を維持する。
- [x] v2の動的評価の入れ子は128までで、再帰経路の超過はbounded/evaluation、通常の不正な深さはprotocolエラー。定義内expression深さ48や既存v1の制限とは別に扱う。
- [x] workはwall timeや厳密なメモリ上限ではない。並列探索のworkと予算境界の部分結果は逐次と同じとは限らない。
- [x] 集合foldの文字列・record、関数key幅等の制限を維持する。ただし要素が1個以下、body が要素を読まない、body が累積値に`\union`で集合を足す（足す集合とIFの条件が累積値を読まない）fold は、文字列・recordを含んでも受理し、固定TLCのログと照合する。累積値に足すfoldのbodyが評価エラーなら従来の拒否を返す。要素が2個以上でmodel valueを含む集合foldは未対応。汎用Valueのvariantの存在だけで対応とは数えない。

[README](README.md)、[IR](src/ir.rs)、[探索](src/safety.rs)、[関数値](src/safety_functions.rs)、[値の登録・移送](src/safety_values.rs)、[時間的性質](src/temporal.rs)、[定数の準備](src/safety_prepared.rs)、[限定symmetry](src/safety_symmetry.rs)、[適用条件](../../docs/symmetry.md)、[並列探索](src/safety_parallel.rs)、[回帰](tests/parallel_safety.rs)、[有限対応の比較入力](../rtla-compiler/tests/finite-support.json)、[対応fragment](../../docs/rust-safety.md)を参照。

[限定POR](src/safety_partial_order.rs)と[optionの制限](../../docs/reductions.md)を参照。
[README](README.md)、[IR](src/ir.rs)、[v2契約](../../docs/safety-ir-v2.md)、[定義検証](src/safety_definitions.rs)、[探索](src/safety.rs)、[時間的性質](src/temporal.rs)、[並列探索](src/safety_parallel.rs)、[回帰](tests/parallel_safety.rs)、[有限対応の比較入力](../rtla-compiler/tests/finite-support.json)、[対応fragment](../../docs/rust-safety.md)を参照。

v2のmembership descriptorは条件分岐の選択枝へ用途を保ち、未選択枝を評価しない。通常の値評価とv1の列挙は維持する。

- [x] 明示v2の非共有anonymous expression/action/temporal frameを扱い、callerのbinder・old・引数を遅延参照する。入口と引数要求へ各1 workを課金し、v1・既存上限を維持する。sourceのLET内の`@`はcompilerがlowerする。

[追加対応と独立比較](../../evals/iterations/0070-tla-coverage.md)を参照。構文受理・有限実行・証明の検証を区別する。

- [x] v2のtemporal定義とTemporalFormula callを受理する。ID・arity・capture・body kind・Next phaseと到達する子を検証し、temporalの循環はrecursive指定でも拒否する。入口と要求のworkはanonymous frameと同じ。v1と既存envelopeは維持する。
- [x] v2の関数選択とDOMAINはalias・actual・IF・record/tupleを通る遅延関数を扱う。所有するcurrent/next/locals/old/prime/frame snapshotから必要なkeyだけを評価し、完全な値が必要なら有限domainを実体化する。値cacheや永続的なlazy値tagを追加しない。
- [ ] EXCEPT target、fold、membershipなど全観測位置での一般の遅延関数互換。通常のScalar経路は全値を評価する。

[追加範囲とworkの区別](../../docs/frontend-semantics.md)、[temporal call検証](tests/temporal_calls.rs)を参照。
