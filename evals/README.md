# 評価しながら改善する

実験の実装・計測・再生は [experiments/](experiments/)、回帰テストは [tests/](../tests/)、固定入力・期待値は [fixtures/](../fixtures/) に置く。実行時にケースを生成するコードは実験に含める。保存済みの計画・実行結果・考察は記録時のパスを保持する。

[0127: rprob の初期候補と遷移ディスパッチ](iterations/0127-rprob-budget-and-dispatch.md)は、予算切れを含む7負荷で候補処理とビット更新を比較した。

[0126: rprob の探索コスト](iterations/0126-rprob-hot-paths.md)は、状態保存・式評価・候補走査・ガード評価を段階ごとに比較した。

[0125: rprob の候補列挙](iterations/0125-rprob-candidates.md)は、試作の準備時間、ピーク RSS、準備済み探索、対応範囲を同じカーネルの変更前後で検査した。

[0124: 実測から採用のゴール証拠を作る](iterations/0124-measured-goal.md)は、固定したモデルで rprob を測定し、改善と測定未確定を採用判定に渡した。

[0123: 検査結果から採用の意味比較を作る](iterations/0123-checker-receipts.md)は、参照評価器と rprob の結果を同じ計画に結び、片方の欠落・旧版の混入・不一致の対照を調べた。

[0122: 自己仕様の採用判断と証拠の版](iterations/0122-evidence-provenance.md)は、節レビュー・意味比較・ゴールの測定を版で照合し、改版後に旧い成功が届く例を検査する。

[0121: 有限集合モデルの探索カーネル](iterations/0121-rprob-kernel.md)は、rprob のビット集合による探索を参照評価器と照合し、時間とメモリを比較した。

[0120: 要件の節を名前付きの検査へ結ぶ](iterations/0120-reviewed-clauses.md)は、rspec の節とレビュー版を発送のモデルに接続し、CLI で検査と対応の変更を報告する。

[0119: 自然文と遷移の対応を追う](iterations/0119-requirement-links.md)は、Event-B の事例から三つのリンク構造を試し、代理読み手の評価と動作を消す修正の対照を行った。

[0118: 発送の仕様と3言語の再生](iterations/0118-shipment-drift-and-replay.md)は、自然言語の要件を遷移の性質に結び、二つの ITF を Rust・TypeScript・Go で再生した。

[0117: 遷移前後の検査と読み取り](iterations/0117-transition-readability.md)は、出荷の二状態の条件を直接記述し、参照評価器・rtlc・TLCの対照と二人の代理読み手で確認した。

[0116: 有限列、値付き variant、公平性](iterations/0116-structured-temporal.md)は、ログ・決済・配信を IR、生成型、rtlc、TLC に通した。既存の Pkl の注文とサービスも変換した。

[0115: Nat と処理前のモデル検証](iterations/0115-nat-validation.md)は、整数範囲の契約を検査器ごとに明示し、Nat の出金を使って実装を呼ぶ前のガードと不変条件の検査を確認した。`just rcheck-validation` で回帰テストを再生できる。

[0114: rmodel の核と移植先の再検証](iterations/0114-rmodel-portability.md)は、出金・Raft・ACL と対照モデルを IR、rtlc、ralloy、TLC、Alloy Analyzer に渡し、表現の限界と検査器間の不整合を区別した。

[0113: 仕様から境界を導き PBT で再生](iterations/0113-linked-boundaries.md)は、分割した出金モデルと3ノードの選挙モデルから境界を選び、Rust と TypeScript の出金実装および故障対照で検査した。`just rcheck-boundaries` で再現できる。

[0112: 条件と更新の読み取り](iterations/0112-rmodel-readability.md)は、同じ三題材の `action` と `behavior` を比較した。rmodel の意味は一致し、代理読み手の正答は26/27と27/27だった。

[0111: 自然言語の仕様から文法の検査を回す](iterations/0111-rmodel-spec-drift.md)は、rmodel の規則と検査を rspec と Jev につなぎ、`behavior` で最初の一周を回した。

[0110: 候補の追加と絞り込み](iterations/0110-more-candidates.md)は、Souther・Koine・Context Mapper・FizzBee・P を加えて12候補を比較した。追加候補では Souther と FizzBee が次の比較に残った。読みやすさは Haiku の週次上限により未測定。

[0109: 表の構文の評価ループ](iterations/0109-surface-eval.md)は、DDD記述・形式化との相性・機械と人間の理解・費用の軸を差し替えられるループを作り、7候補で回した。軸ごとの数は0107の結論と同じ形で、人間の軸は満点に張り付いて候補を見分けなかった。

[0108: 核のIRのJSONと、任意のadapter、roracleでの差分テスト](iterations/0108-ir-adapters.md)は、rmodelの構文木をJSONの契約にし、PklからIRを書くadapterの判定をroracleでrmodelの文と照らした。写せる4題材で一致し、adapterの人工故障2件を検出した。

[0107: モデルの書き方の比較](iterations/0107-surface-languages.md)は、5つの題材をrmodel・Quint・OCaml・Isabelle・Idris 2・Pkl・Veilで書いて検査した。すべてSPECと一致し、詰む場所は系統で分かれた。ホストの関数で書くと解析できず、SMTに渡す言語は断片で詰む。

[0106: 帰納法の検査器の試作（proto）](iterations/0106-induction-proto.md)は、核のIRからVCを作りz3で帰納的な不変条件を検査する試作で、CTIを参照評価器で検査し直す。直した後のcorpusと生成したモデルで食い違いは0件、断片は狭い（生成したモデルの4%）。

[0105: MoonBitのtraitとdecodeの生成](iterations/0105-moonbit-bindings.md)は、同じJSONの契約でMoonBitのtrait・厳密なdecoder・`Server::handle`を生成し、MoonBitの2実装の照合と、3件の人工故障の検出を確かめた。

[0104: Goのinterfaceとdecodeの生成](iterations/0104-go-bindings.md)は、同じJSONの契約でGoのinterface・厳密なdecoder・driverを生成し、Goの2実装の照合と、3件の人工故障の検出を確かめた。

[0102: ドメインのモデルから型を生成し、実装を照合する](iterations/0102-domain-bindings.md)は、モデルからRustのtrait・TypeScriptの型・JSON Schemaを生成し、参照評価器を正本に実装を照合した。Rustの3実装・TypeScriptの2実装が一致し、3件の人工故障を両言語で最短の列で検出した。

[0103: 値の種類（整数・bool・record）](iterations/0103-values.md)は、`Int`・`Bool`・`record`を値として入れ、整数が範囲の外へ出たら未確定と答えるようにした。生成した600モデル・1800 checkで参照評価器と明示状態の判定は食い違わず、TLA+の直積の写し方を変えてrtlcのエラーが0件になった。

[0101: 検査器の共通の枠と、1つの入口](iterations/0101-checker-registry.md)は、crate `rcheck`で、検査器ごとにcomplete・bounded・対応しないを宣言させ、`auto`・`all`で答えを用語集の語で返した。corpusの32 claimで一致し、0100でcrashした2 seedから、参照評価器の予算の数え方の穴を直した。

[0100: 和のsort（多態のentity）](iterations/0100-sum-sorts.md)は、`type Party = Person + Org`で、型検査で書けなくなった多態の表現を部分型として戻した。生成した598モデル・1794 checkで3つのengineの判定は食い違わず、crashした2つのseedは参照評価器の予算の数え方の穴だった。

[0099: sortの静的な型検査と、多重度の違反の報告](iterations/0099-sorts-and-violations.md)は、rmodelにsortの型検査を入れ、多重度を破る遷移を`Declarations`の違反として報告するようにした。生成した600モデル・1800 checkで3つのengineの判定は食い違わず、読み手の誤答からsortの合わない式が消えた。

[0098: 文法の候補を、意味と結果が読み取れるかで評価するループ](iterations/0098-syntax-readability.md)は、Quint風のprinterと、参照評価器から問いと正解を作る仕組みを作り、文脈のない読み手で42問中36問の正答を得た。誤答6件はすべてsortの合わない式で、静的な型検査の必要を示した。

[0097: rmodelの参照評価器と、写し方の差分テスト](iterations/0097-rmodel-reference.md)は、意味論の正本として参照評価器を作り、生成した600モデル・1200 checkで参照評価器・rtlc・ralloyのSATを比べた。判定の食い違いは0件で、rtlcのバグ（#112）が見つかった。

[0096: SATの経路で整数の桁あふれを検出し、判定を未確定にする](iterations/0096-quint-overflow.md)は、状態に依存する足し算・引き算・符号反転の桁あふれを`c_overflow`で検出するようにした。桁あふれで偽の反例が出るモデルで、それを検出して未確定にできる。

[0095: QuintのモデルをSATでも有界に検査し、明示状態の経路と比べる](iterations/0095-quint-sat.md)は、Quint IRをAlloy 6へ写し、0044の12件のうち10件でralloyのSATとAlloy 6.2.0がTLCと一致した。5 bitに収まらないSequenceFoldは拒否した。状態の少ないモデルでは、rtlcがSATより2〜4桁速かった。

[0094: roracleの`compare`を語彙の上で記述し、実装を照合する](iterations/0094-oracle-compare.md)は、観測の組から比較の結果を出す規則をモデルにし、81通りで実装を照合した。enumでsigが増えたことで、ralloyの型検査がsig 15個以上で桁あふれするバグが見つかり、直した。

[0093: 語彙の上で実装の振る舞いを書き、実装を照合する](iterations/0093-implementation-behavior.md)は、roracleの比較の集約をrmodelで記述し、比較0〜3件の156通りで実装の結果をモデルのinstanceとしてralloyの`check_instance`で照合した。実装の故障2件は照合が、モデルの故障はclaimの検査が検出した。

[0092: ユビキタス言語と、層の間のdrift検知](iterations/0092-ubiquitous-language.md)は、Pklで用語・rule・claimを書き、モデルの宣言・コードの記号・checkへ結びつけた。決定的な検査はrspec、意味の食い違いはjev-lint docsで見る。予算切れをralloyはunknown、rtlcはboundedと呼ぶ語の違いが見つかった。

[0091: 構造と振る舞いを1つのモデルで書く言語の核](iterations/0091-rmodel-language.md)は、静的な関係・状態・暗黙のframeを持つactionを1つのモデルで書き、AlloyとTLA+へ独立に写した。5モデル・20 checkでralloyのSAT・rtlc・Alloy 6.2.0・TLCが期待と一致した。設定の空間を持つ権限モデルでは、SATが明示状態より2桁以上速かった。

[0090: ralloyの時相をSATで有界に検査する](iterations/0090-ralloy-temporal-bmc.md)は、Alloy 6のvarと`always`・`eventually`・`after`・primeを、長さstepsのlassoとしてSATへ展開した。時相の25 commandがAlloy 6.2.0と一致し、SATのtraceはすべて評価器の検査を通った。

[0089: Quint IRを直接読むfrontend](iterations/0089-quint-ir-frontend.md)は、Quint CLIのJSON IRを標準のTLA+へ写すcrate rquintを作った。Apalacheを経由せずに、0044の12件でTLCと判定・状態数・生成数が一致し、builtinの事実もQuintのsimulatorと同じ判定になった。

[0089: 順序に依存しない集合 fold をrecord・文字列を含む要素でも受理する](iterations/0089-record-fold-order.md)は、TLCのintern IDなしで結果がTLCと同じになると分かる3つの形（要素1個以下、bodyが要素を読まない、累積値への`\union`）だけをrtlcが受理する。固定TLCで22入力を採取し、宣言した9件と新しい対照がログと一致した。今受理される入力の結果・work・IRは変わらない。

[0088: 総当たりとSATのportfolio](iterations/0088-ralloy-portfolio.md)は、総当たりを5000 workで先に走らせてSATへ切り替える方法を実装し、正しさの検査を通した。速度の2回の測定は関係のないプロセスの負荷（8〜22）に埋もれて`hold`になり、変更を戻した。

[0087: SATと総当たりの切り替えの境界](iterations/0087-ralloy-backend-threshold.md)は、171 commandで両backendを測った。合計が最小の4 bitは、小さい探索を2〜13倍遅くして`hold`になり、24 bitに戻した。幅は、解がすぐ見つかるかを表さないため、backendの選択の目安にならなかった。

[0086: SAT backendでsigの階層と整数を扱う](iterations/0086-ralloy-sat-hierarchy-integers.md)は、階層を所属の変数と加算回路の数の制約で、整数をbit幅のbitベクトルで符号化した。`div`・`rem`以外のモデルをSATで解け、166 commandで総当たりと一致した。速度は負荷の中で2回とも`hold`。

[0085: ralloyのSAT backend](iterations/0085-ralloy-sat-backend.md)は、関係の中核をAIGとTseitin変換でCNFにし、ralloyの中に書いた小さなCDCLで解く。総当たりでwork 25.6億だった`CompositionHolds`がwork 1,910で解け、174 commandすべてでAlloyと一致した。

[0084: ralloyの対称性の除去を探索の途中で行う](iterations/0084-ralloy-incremental-symmetry.md)は、枝を途中で刈り、2列以上のfieldとsubset sigにも広げた。`CompositionHolds`の候補は約15分の1になったが、予算内では解けない。数µsのケースが最大1.22倍で`hold`。

[0083: ralloyの対称性の除去](iterations/0083-ralloy-symmetry.md)は、lex-leaderで入れ替えられるatomの重複した候補を評価前に除き、`core-set-algebra`を128 msから34 msにした。0078からの累積では8ケースが0.30〜0.64倍になり、数µsの4ケースは最大1.17倍のまま残る。

[0082: ralloyの整数とcardinality](iterations/0082-ralloy-integers.md)は、整数・`#`・算術・比較を加えた。Alloy 6.2.0の既定の2の補数の桁あふれを答えから確かめて合わせ、31 commandすべてで一致した。速度は`adopt`。

[0081: ralloyの引数付きpred/fun](iterations/0081-ralloy-parameterized-predicates.md)は、引数付きpred/fun、receiver形式の呼び出し、box join、引数付きpredの`run`を加え、23 commandすべてでAlloyと一致した。速度は`adopt`。

[0080: ralloyのsig階層](iterations/0080-ralloy-sig-hierarchy.md)は、abstract・extends・subset sig・一括宣言・signature factを加え、40 commandすべてでAlloyと一致した。子のscopeの扱いはAlloyの答えから決めた。速度は小さいケース1つが2回とも閾値を超え、`hold`として記録した。

[0079: ralloyのn項関係](iterations/0079-ralloy-nary-relations.md)は、4項までの関係と矢印の多重度を加え、Alloyとの差分で型の交わらないjoinの非互換を見つけて直した。`nary`は30 commandのうち29でAlloyと一致し、1件は予算切れとして残した。速度は二項以下で5〜13%遅くなり、`hold`として記録した。

[0078: ralloyとAlloyの差分テスト](iterations/0078-ralloy-alloy-oracle.md)は、公式Alloy 6.2.0の答えをroracleの`CheckReport`として保存し、`cargo test`で再生する。今の対応範囲の13モデル・50 commandがすべて一致し、評価器の人工故障6件を検出した。

[0077: 増分contextの固定費の削減](iterations/0077-smt-lean-context.md)は、profileに合わせて、文字列を作らない長さの計算、遅延したsource hash、遅延したProblem、名前を含む式だけの再コンパイルを入れた。`context-assert-256`は0.59倍、`context-constant-256`は0.47倍。単発の小さいparseの後退も戻った。旧版との完全応答一致とZ3照合を記録し、cache benchの`prepared-4`の1組の後退も残した。

[0075/0076: check cacheの往復費用とengine側のstore](iterations/0076-smt-cache-store.md)は、JSON/Wasm protocolでsnapshotを往復させる費用を測り、N=512でhitがcacheなしのcheckの21〜28倍になることを確かめた。engine側のstoreを追加し、hitはNによらずnative 4.6 µs・Wasm 9.9 µsになった。snapshotを往復させた場合との2400要求の完全応答一致も記録した。

[0075: 連鎖をcheckerの検証と評価でも1段と数える](iterations/0075-nesting-chains-ir-eval.md)は、rtlcのv2で連言・選言とELSE IFの連鎖を明示的なloopでたどり、checker 48と動的評価128の1段と数える。54枝のLongIfChainはTLCと同じ56状態・57生成で検査でき、今受理される入力のIR・結果・workは変えない。評価の再帰の腕を分け、9種類の入れ子がdebugの2 MiBのthreadで動的評価128に届くようにした（#83）。JSONの要求のdecoder 128により、平たい連言127項とCASE 126枝からは型付きで拒否される。

[0074: SMT contextの増分解析とcheck cache](iterations/0074-smt-incremental-cache.md)は、assertで追加した式だけを解析する変更と既定無効のcheck cacheを、bcf018be1との交互順Criterion、旧native/Wasmとの完全応答、Z3照合で評価する。保存runで見つけた2回の後退と追記plan、0074bの`prepared-128`の1組の後退と、0074dの再測定で再現しなかったことも記録した。

[0074: 連言・選言とELSE IFの連鎖](iterations/0074-nesting-chains.md)は、Quint生成物の長い括弧つき連言とIF連鎖を、IR v2の両frontendで1段の入れ子として数える。44項の連言はTLCと同じ6状態・7生成で検査でき、連鎖をたどる処理は段ごとに再帰しなくなり、61段の連鎖もdebugの2 MiBのthreadでcompileできる。今受理される入力のIRは4,651入力・27,906セルで一致し、完全応答と時間論理の照合も一致した。Criterionに再現する後退はない。54枝のIF連鎖はcheckerの48で止まり、0075で扱う。

[0073: 後続が読まない束縛の再利用](iterations/0073-dead-binding-reuse.md)は、`nondet` を重ねたQuint生成物で同じ後続を何度も評価する費用を除く。合成モデルのnative 1/2/4/8 workerで約0.48〜0.52倍、状態・生成数・反例は一致し、workは変更前を超えない。2回の独立レビューで見つかった6件と対応も記録した。既存corpusと14本のCriterionに後退はない。

[0072: source level・遅延関数・template共有](iterations/0072-frontend-semantics.md)は、SANY/TLCの拒否・正常/故障対照とnative/Wasm/Chromiumを照合する。共有化前後の完全応答・workとCriterionのcompile/fresh/preparedを分け、入力条件の修正と失敗も保存する。

[0070: TLA+ frontendの追加対応](iterations/0070-tla-coverage.md)は二版のSANY構文木、native/Wasm/TLCの実行と反例、再帰の予算切れを比較する。高階・module・proofの構文保持と有限実行の範囲を区別する。

[0064: 準備済みSafetyProgramとbatch](iterations/0064-prepared-safety.md)は同じIRへ予算を変えて問い合わせる費用を比較する。小さいqueryで採用し、探索主体のモデルでの効果の小ささ、cold/RSSと既存kernelの後退確認を残す。

モデル検査・テスト手法の採用判断を記録する場所。仮説、条件、実行、判断、次の仮説を1つの iteration として残す。比較器の `improved` は故障ごとの検出数の増加を表し、費用や外的妥当性を含む採用決定ではない。

[0062: 共有IRと最新版の比較](iterations/0062-p0-checker.md)はdraft実装を0061へ統合し、4モデル9経路、Quint生成物と原本TCommit、IRの線形成長、profileとv1の費用増を比較する。静的visitorは撤去し、共有IRは明示選択で採用する。

[0063: Data定数のslot検索](iterations/0063-evaluator-tuning.md)は内部addressのhashだけを変更し、TCommitのnative検査要求・起動を短縮した。14本に残る数%の後退と、membershipの後退で撤去したinline、v1/v2の完全応答比較を記録する。

[0061: symmetry拡張と探索削減](iterations/0061-symmetry-next.md)は、record・集合、代表展開、並列化、命令ID、限定PORを順に試した。命令IDは後退して撤去し、採用した条件・具体反例の再生・計数の違いを記録する。

[0053: TLCの24不一致解消](iterations/0053-tlc-standard-modules.md)は標準moduleを固定して参照pathを渡し、int32最小値のtrace表記も修正した。155件の検査・32件の確認・155件の保存再生で不一致0を確認した。

[0052: workspaceのoracle統合](iterations/0052-workspace-oracle.md)は23ファイル・99比較をRust/Wasmへ通し、3言語と保存反例を再検査した。source/binary snapshotからのbefore/after比較と再生、JSON bridgeの追加費用、当時のTLCの24不一致も記録する（0053で解消）。

[0051: roracle](iterations/0051-oracle-library.md)は、固定入力・実装登録を差分検査・保存oracle再生・比較ベンチで共有する独立crateを追加した。TLC/tla-ts/idealisのcold比較とprepared Rustを分け、人工故障・測定中drift・clock分解能による最初の失敗も記録した。

[0050: アクション単位のcoverage](iterations/0050-action-coverage.md)は、Rust/TS/native/Wasmへopt-inのbranch計数とwork内訳を追加した。162入力と840回のon/off照合、TLCの14仕様、3反例の再生、旧APIとの完全応答比較を記録した。式・source location・並列のcoverageは未対応。

[0049: 時間論理のmicro tuning](iterations/0049-temporal-tuning.md)は、profileに基づき小さい式集合をbit集合へ置き換えた。交互測定で時間論理のcheckは1.35-1.42倍となり、156ケース・各10要求でbefore/afterの完全応答を照合した。対照8件に残る後退も記録している。

[0048: 量化fairnessと標準演算](iterations/0048-fairness-operators.md)は、SPECIFICATIONの定数有限domain上のfairnessとSelectSeq / SortSeq / Assertを追加した。52ケースのnative/Wasm/TLC比較、4反例の再生、Chromiumと27本のCriterionで対応と費用を検査した。

[Rustの状態展開cache](iterations/cached-expansions.md)は、現在のInit/propertyを再検査し、同一Nextの完全なsuccessor列を再利用する。Leanの追加9定理、実Wasmの16条件、native/Wasm/Chromiumと36区間のCriterionで、効く場合と固定費が上回る場合を分ける。

[完了結果の共有キャッシュ](iterations/verified-cache.md)は、Leanの17定理、TSとの23,606遷移の比較、TLCの故障対照、実SQLite/HTTP/Chromiumで再利用条件を検査する。task単位の任意cacheとして採用し、状態単位のインクリメンタル探索は分けて扱う。

[Quint生成物のv1/v2回帰と測定](iterations/quint-sharing-regression.md)は、6実生成物をnative/Wasm/Chromiumで検査し、TLCへ24結果と反例12件を照合する。42区間のCriterionでは、小さいモデルでv2の固定費が上回る結果も保存する。

[0048: operator定義の共有](iterations/0048-operator-sharing.md)は、safety IR v2で式定義を共有する変更を、実装の前に置いた回帰とRedで検査する。既存239入力と追加17入力でv1の出力が変わらないこと、17入力のTLC照合、両frontendの解析の一致（4236件）、深い公開IRの破棄で許す確保の上限を扱う。

[0047: TLA+モジュールと設定置換](iterations/0047-module-linking.md)は、外部EXTENDS・引数なしINSTANCE・LOCAL・WITH・設定置換を扱う。37ケースのnative/Wasm/TLC比較、3反例の再生、Chromiumと17本のCriterionで対応と初期費用を検査した。

[0046: TLCの有限モデル対応](iterations/0046-tlc-finite-support.md)はmodel value、探索制約、有限CHOOSE、整数演算、定数有限domain上の時間論理量化をTLC/native/Wasmへ照合する。既存14本のCriterionと、追加6モデルのcompile/checkを分けた12本で費用を測る。

[0045: crateチェックリストの同期](iterations/0045-crate-checklists.md)は、Jevによる宣言した28比較と、staged snapshotへのオフライン再生を確認する。未確定の記録、人工的な矛盾2件と正常対照、部分stage・古い記録・配布artifactも検査した。

[0044: Quint生成TLA+](iterations/0044-quint-generated.md)は、実コンパイラの6生成物・12設定をTLC/native/Wasmで照合する。生成物を変更せず、反例のTLC再生、Chromiumと保存fixtureの回帰を確認する。

[0042: SMTのCriterionと解析費用](iterations/0042-smt-tuning.md)は、14ケースの固定benchmarkで不変な解析結果の共有を評価する。profile、改善前後の順序を入れ替えた測定、ばらつきの再確認、旧版との完全応答とZ3照合を記録した。

[0030: RustのSMTサブセット](iterations/0030-smt-subset.md)は生成入力とSATモデルをZ3に照合する。[0031: Z3の再利用を含む速度比較](iterations/0031-smt-reuse.md)でnative/Wasm、解析、prepared、push/pop、起動の費用を分けた。限定した差分制約の経路は候補とし、Boolean UNSATの後退とunknownも保存している。

[0034: 静的Alloyサブセット](iterations/0034-alloy-subset.md)は、独立crate `ralloy`のnative/Wasmと公式Alloy 6.2.0を比較する。有限scopeの関係モデル、返却構造の受理、空関係のarityの修正、ブラウザと単独consumerを検査した。

[0035: Alloyモデルと固定instance](iterations/0035-alloy-instance.md)は、実装の関係スナップショットをnative/Wasmで評価する。公開TS API、node:test/Vitest、全512グラフ、公式Analyzerへの構造固定、実Chromiumと単独consumerを検査した。

[0036: Rust testing core](iterations/0036-testing-core.md)は、生成・縮小・差分比較・保存caseの再生を独立crateへ移した。同じcorpusでnative/Wasm、TS生成器と既存縮小器を照合し、状態付きカウンタとブラウザ・配布consumerを検査した。

[0038: 明示した並行スケジュール](iterations/0038-controlled-schedules.md)は、actor列の生成器をnative/Wasmで共有する。TSのasync処理とgateした実Rust threadを同じ順序で進め、逐次対照に対するlost updateの追加検出、Loomの参照、縮小と再生を検査した。

[0041: SMT-LIB script](iterations/0041-smt-scripts.md)は、push/popと複数checkをZ3の逐次実行と独立snapshotに照合する。共有work予算、後続エラー、空結果、native/Wasmの一致と再生を検査した。

[0040: SMTの式とcontext API](iterations/0040-smt-expansion.md)は、定数倍・整数ite・assert/push/popと一時Bool assumptionsをZ3に照合する。native/Wasmの旧版との差分、SATモデル、独立したassertion stackと再生を検査した。

[0039: 構造付きfuzzing](iterations/0039-structured-fuzz.md)は、testing protocolと保存caseをASanで検査する。生JSONLとの到達性比較、状態破損を注入した対照、保存corpusのnative/Wasm再生を記録した。

[0037: SAT/CNF候補](iterations/0037-sat-foundation.md)は、Varisatのnative/Wasmを独立全代入列挙へ照合した。実Chromiumでも動く候補として残し、solver内部の実行バジェットを保証するbackendへの採用は保留している。

```sh
just eval-loop
just replay-loop
```

`eval-loop` は既定の plan で開発用・確認用 seed を実行し、新しい run ID を作る。`replay-loop` は台帳で最新の完了した確認 run を選び、故障版で失敗・正常版で通過することを検査する。

plan と run ID を明示する場合:

```sh
just eval-loop evals/plans/0001-longer-walk.json my-run
just replay-loop evals/runs/my-run-confirmation
```

| 場所                       | 記録するもの                                                       |
| -------------------------- | ------------------------------------------------------------------ |
| `methods.md`               | 手法の定義、比較単位、前提、解釈の制約                             |
| `plans/`                   | 結果を見る前に固定する仮説、変更点、予算、seed、判断の観点         |
| `runs/<id>/manifest.json`  | plan、ソースの SHA-256、revision、dirty paths、実行環境、親 run    |
| `runs/<id>/trials.json`    | 正常版と全故障の seed ごとの検出・見逃し・コスト・再現結果         |
| `runs/<id>/corpora.json`   | 生成ケース数、状態・遷移、生成時間、要求操作数                     |
| `runs/<id>/witnesses.json` | 故障版を元のモデルで検査し直す操作列                               |
| `runs/<id>/result.json`    | 集計、正常対照、評価、各証拠の digest。完了の目印                  |
| `runs/ledger.jsonl`        | run の追記台帳。例外は `error`、正常対照の失敗は `controls-failed` |
| `iterations/`              | 結果の解釈、採用・保留の理由、後退、次の仮説                       |
| `experiments/harness/`     | 実験用のモデル・実装・探索器。public API と分離                    |

## ループの進め方

1. 前の iteration の見逃し・費用・調査の難しさから、変更点を1つ選ぶ。
2. plan を作り、結果を見る前にコミットする。モデル、正常版、故障集合、予算、開発用・確認用 seed を明示する。
3. 実装とテストを整え、開発実行を記録する。
4. 同じ plan / ソースのまま確認用 seed で実行する。途中の変更、重複 seed、変更された証拠を拒否する。
5. 数値だけでなく witness を再現する。正常版の誤検出、無効操作、再現失敗は判断の前提を崩す。
6. iteration に判断を追記する。後退がある場合も保存し、次の仮説へ進む。

run には iteration が引用する summary（表、集計、manifest、終了コード、sha256）だけを commit する。Criterion の sample、ケースごとの出力、log、大きな before / after の dump は commit せず、必要なら消したファイルの数・バイト数・digest を `reduced.json` に残す（例: [0074](iterations/0074-nesting-chains.md#記録の縮小)）。`just replay-loop` が読む run（`result.json` のある eval-loop・確認 run）は `trials.json`・`corpora.json`・`witnesses.json` を含め、縮小しない。digest は、元の記録を持つ者が照合するためのもので、commit した記録だけでは消した内容を検証できない。

既存 run ID は上書きできない。中断や例外で `result.json` がない run を合格として扱わない。結果・台帳を編集して再試行を隠さず、新しい ID で実行する。formatter の対象から `runs/` を除外し、元の証拠を保つ。

確認用 seed も同じ人工故障集合を使う。これはスケジュールに対する確認であり、未知故障や実システムへの一般化を検証したことにはならない。確認結果を見て次の仮説を作る場合は、別の確認用 seed を予約する。

実行済みの比較は [0001: 履歴長16](iterations/0001-longer-walk.md)、[0002: 8/16の混在](iterations/0002-mixed-depth.md)。次の仮説は [backlog](backlog.md) に残す。CI で同じ plan を反復するのは再現・回帰確認であり、新しい独立した確認実験として集計しない。

初回の幅広い pilot は `just evaluate` / `just replay-evaluation` で再実行できる。履歴は [0000](iterations/0000-pilot.md)。古い記録には後付けの厳密な provenance を主張せず、`legacy-import` として区別した。

実行時間と Rust 化は [0003](iterations/0003-runtime.md)。[runtime/](experiments/runtime/README.md) に budget planner の前提、Rust/Wasm の比較、クラウド分割 CLI を記録している。

[0007 の計画](plans/0007-shared-oracle.md) は、同じ仕様 oracle で TS / Rust / Go の実装を直接操作する。`just cross-test`、`cross-eval` / `cross-replay` と [利用手順](../docs/shared-oracle.md) を参照。

[0008 の結果](iterations/0008-async-tuning.md) は、未確定操作を含む非同期履歴、障害境界、model handle、trace buffer、依存縮小、feedback 探索の採用判断。`just async-test`、`async-eval` / `async-replay` と [履歴の意味・API](../docs/async-history.md) を参照。

[0009〜0011 の結果](iterations/0009-rust-safety.md) は、TLA+ safety fragment の状態生成・BFS を Rust native / Wasm 内で行う backend と、literal decode・状態保持の改善。TLC32件、変更前後37入力、各段階の固定入力測定を保存した。`just safety-test` と `safety-eval` / `safety-replay`、[対応範囲](../docs/rust-safety.md) を参照。

[0012 の計画](plans/0012-collections-temporal.md) は、有限集合・関数・record・sequence、有限量化、temporal property と WF / SF の Rust backend 対応。`just complex-eval` / `complex-replay` は入力・結果・反例・対照を保存して再現する。`complex-tlc` / `complex-tlc-replay` は同じ入力と強制した反例を TLC で検査する。`safety-eval` は以前の6 scalar 入力と4つの複合値・temporal 入力を測る。

[0012〜0014 の結果](iterations/0012-collections-temporal.md) に、複合値・temporal の対応、49入力・TLC46入力・反例14件、整数比較と intern table の改善、一律の高速化には至らない条件を記録した。

[TLC直接比較の結果](iterations/0015-tlc-differential.md) は、境界値と seed 固定の小さい有限モデルを TLC / Rust native / Wasm へ渡す差分検査。`just tlc-diff` / `tlc-diff-replay` で判定・計数・返却値・反例を確認する。TLCを拒否対照、モデル評価エラー、実行基盤の失敗と分け、JVMごとに標準モジュールの展開先を隔離する。

[速度比較の結果](iterations/0016-checker-benchmark.md) は、同じ11モデルを tla-ts / TLC / idealis native / Wasm で完全探索した測定。`checker-bench` / `checker-bench-replay` で起動込みとAPI再利用時を分け、source解析を含む時間・状態数・生成数・生ログを保存する。

[常駐poolの評価](iterations/0017-parallel-safety.md) は、独立したTLA検査を1/2/4/8 workerへ配る。`parallel-safety-test` / `parallel-safety-eval` / `parallel-safety-replay` で、予算によるworker数の選択、費用順配置、batch化、結果と反例の一致を確認する。1モデル内部のBFS並列化とは区別する。

[TLCとのbatch比較](iterations/0018-tlc-throughput.md) は、同じ88件を指定探索worker合計8で実行する。TLCの1 JVM × 8 worker / 8 JVM × 各1 workerとidealis native / Wasmを比較し、JVM起動とIR再利用を含むthroughputの差を記録する。`tlc-throughput` / `tlc-throughput-replay` で実行できる。

[単一モデルの比較](iterations/0019-single-model-scaling.md) は、約2,100万遷移の幅広い3モデルと幅1の対照を使い、TLC 1/2/4/8 workerと逐次native/Wasmを測る。`single-model-test` / `single-model-eval` / `single-model-replay`で正しさ・全体時間・補助区間・RSSを保存する。TLC内部方式の移植検討は[並列探索の調査](../docs/tlc-parallel-adoption.md)を参照。

[nativeの層同期BFS](iterations/0020-frontier-safety.md)は、単一モデルを並列展開する明示的な経路。共有workカウンタの競合による後退と、初回の幅1測定への干渉も記録する。`frontier-safety-test` / `frontier-safety-eval` / `frontier-safety-replay`で検査する。

[workerごとの予算予約](iterations/0021-worker-budget.md)は、共通counterへの毎tickの競合を減らす。同APIの1→8 workerで関数・チャネルは中央値約1.8倍、counterは遅くなるため、明示的な経路として採用する。`frontier-budget-eval` / `frontier-budget-replay`で固定入力の測定・再生を行う。

[READMEの6経路比較](iterations/0022-readme-comparison.md)は、同じ4入力をtla-ts 1、TLC 1/8、Wasm 1、native 1/8で測り直した。条件・全sample・再生とREADMEへの照合を保存する。

[Criterionによる最適化](iterations/0023-criterion-safety.md)は、値の一重保持、EXCEPT、型所属cache、候補buffer、重複検査、worker/batch選択の6項目。`criterion-test` / `criterion-save`でnative kernel/APIを測り、`criterion-compare` / `criterion-replay`で固定binaryの前後・起動・RSS・正常/故障を確認する。Wasm scalarの後退も記録した。

[CPU profileとmicro tuning](iterations/0024-profile-tuning.md)は、Wasm/nativeのraw profileとself/inclusive集計からleaf評価と整数位置検索を調整した。`profile-record` / `profile-summary`でprofile、`profile-criterion`で14本、`profile-scalar` / `profile-replay`で通常Wasmの交互比較と再生を行う。

[rtlcへの分離](iterations/0025-rtlc-extraction.md)は、有限TLA+ fragmentの検査をidealisから独立したcrateへ移した。`rtlc-test` / `rtlc-example`と単独package consumer、分離前後のnative964件・Wasm241件で既存挙動を確認した。探索アルゴリズムと速度の主張は追加していない。

[rtlaのparser](iterations/0026-rtla-parser.md)は、TLA+／TLC設定を型付きASTへ解析する独立Rust crate。`rtla-test` / `rtla-browser`でnative/Wasm、`rtla-eval`で199組のASTとSANYの構文対照を検査する。IR生成はTS側に残し、対応fragmentと全TLA+を区別する。

[Apalacheとの比較](iterations/0027-apalache-comparison.md)は、BMC/帰納性と有限グラフのcompleteを区別する。`apalache-test` / `apalache-eval`で同じsource/configのpilotを実行し、raw log、ITF、IR、反例検証と起動込み時間を保存する。

[backend契約](iterations/0028-backend-contract.md)は、explicit complete、有界の反例なし、帰納性と個別obligationを分ける。`backend-test` / `backend-eval`で実rtlc native/Wasm、0027の保存したApalache応答、非空性と帰納性の集約条件を確認する。新しい速度比較や汎用Apalache adapterの評価ではない。

[Rust frontend](iterations/0029-rtla-compiler.md)は、rtlaのASTからrtlcのIRを生成する。`compiler-test` / `compiler-eval`で既存TS frontendとのIR・結果の一致、`compiler-tlc`で固定20件の直接比較を行う。意味の一致を確認する回帰で、性能や未知故障発見の実験とは区別する。

[0054の結果](iterations/0054-oracle-bench-tuning.md)は、workspace比較のpipe bufferingとoracle JSON境界の借用・直接serializationを、旧版を固定して評価する。通信・照合kernel・モデル検査の費用を区別する。

[0055の結果](iterations/0055-oracle-hotspots.md)は、Node CPU profileから観測JSONの二重構築とsnapshotの中間配列を減らす。区間別測定、同じ親runnerでの往復、Criterionを分けて比較する。

[0056: 関数値と並列domain共有](iterations/0056-compact-functions.md)は、domain共有・64要素のBool/整数bit表現とworker参照を実装し、旧native/Wasm・TLC・tla-tsへ同じ完全探索を比較する。初期候補の並列後退、最終候補の改善と残るWasmの遅さを分けて記録した。

[0057: 重複する関数更新のコピー削減](iterations/0057-probe-updates.md)は、borrowed viewで更新後の既存値を検索する。Aの関数並列の後退、Bのチャネル並列の改善、Wasmに残る差を、同じ条件のbefore/after・TLC・tla-tsで記録した。

[0058: packed関数のhash](iterations/0058-packed-hash.md)は、公式C++のgolden vectorへ照合したfmix64で内部索引を軽くする。functionのnative/Wasmの改善、対照の後退、負荷変動でチャネルの速度判断を見送った条件を記録した。

[0059: prepared定数・型別命令・並列照合](iterations/0059-prepared-execution.md)は3案を順に試し、Data定数cacheを採用する。型別命令と並列照合の後退、最終9経路と14本の段階別比較、旧版への完全応答照合を残す。チャネルのWasmは改善後もtla-tsより遅い。

[0060: 限定したSYMMETRY](iterations/0060-restricted-symmetry.md)は、scalar node関数の行で状態を同一視する。宣言の前提、nativeの起動込み・prepared比較、TLCと具体traceの照合、通常ringで残る変動、失敗した測定driverも保存した。

- [0071: 再帰action生成と親frame共有](iterations/0071-recursive-actions.md): TLC・native/Wasm・ブラウザの照合とCriterionの測定。
