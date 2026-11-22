# 評価しながら改善する

実験の実装・計測・再生は [experiments/](experiments/)、回帰テストは [tests/](../tests/)、固定入力・期待値は [fixtures/](../fixtures/) に置く。実行時にケースを生成するコードは実験に含める。保存済みの計画・実行結果・考察は記録時のパスを保持する。

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
