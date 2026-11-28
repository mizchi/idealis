# idealis

日本語 · [English](README.en.md)

仕様を起点に、モデルの性質と実装の振る舞いを検査する、ポータブルな形式モデル駆動のツールキット。

[Idealis の仕様を読む](spec/README.md) と、最初の `.ideal` モデル、検査の実行方法、処理系の構成をたどれます。

**このリポジトリは開発途中の未完成なツールキットです。** 現在は`0.1.0-alpha.0`の試作段階で、API・対応範囲・backend間の統合を整備しています。npm / crates.ioには未公開です。

設計の軸は、値や制約を扱うZ3/SMTと、状態遷移や並行実行を扱うTLA+。この二つを相互運用できるライブラリとして用意し、モデル検査・オラクル生成・実装照合を各テストツールへつなぎたい。最初の対象言語はTypeScript、Rust、Go。[目指す構成と進める順序](docs/vision.md)を参照。

## 互換性と API の安定性

`rtla`、`rtla-compiler`、`rtlc` は、既存の TLA+ モデルや TLC 設定をそのまま使える範囲を広げ、解析・検査を高速化することを目指す。`rusmt` も、対応する SMT-LIB 入力を増やし、求解性能を改善していく。互換性の基準は、入力を受理できるか、その意味や検査結果が参照実装と一致するかにある。現在の対応範囲は各 crate の [チェックリスト](docs/crate-checklists.md) に記録している。

`idealis-cli` が提供する `idealis tla` と `idealis tlc` は、対応する SANY / TLC のオプションや終了コードとの互換性を重視する。一方、モデルと実装をつなぐ `idealis-connect` や TypeScript SDK などの高レベルなラッパーは、抽象度が上がるほど API が変わりやすい。アルファ版の更新時には、関数の引数や Driver の型など、呼び出し側の変更が必要になることがある。[CLI の対応範囲](crates/idealis-cli/README.md#tla-ファイルを検査する)

## 目指す開発体験

1. 状態・操作・invariant・環境仮定を仕様に書き、仕様モデル自体の性質を検査する。
2. モデルと制約から、テストの入力列と許容する観測を作る。
3. 各言語のDriverで実装を動かし、障害や実行順を変えながら、観測履歴が仕様の許す振る舞いに含まれるか照合する。
4. 逸脱したcaseを縮小・保存し、仕様や実装を修正した後も同じ反例をCIで再生する。

中心に置くのは、[Quint Connect](https://github.com/quint-co/quint-connect)のようにモデルと実装をDriverで結ぶ使い方。期待値は仕様から作り、実装の観測から作らない。非決定的な仕様では、観測に整合する候補を残して検査する。

実装に埋めたinvariantや事前・事後条件も、言語別frontendから共通の検査基盤へ渡せるようにしたい。[Why3](https://why3.org/)の検証条件へ集約する構成を参考にする。現在の`Model.invariants`はモデル状態を評価する機能で、ソース注釈の抽出や任意コードの静的検証は今後の計画。

## 最初の対象言語

| 言語       | テストへの接続                | 現在の主な経路                                                 |
| ---------- | ----------------------------- | -------------------------------------------------------------- |
| TypeScript | Vitest、`node:test`、ブラウザ | Model/Driver、Rust/Wasmコア、PBT・差分テスト・再生、Z3 adapter |
| Rust       | `cargo test`                  | nativeの共通コア、独立crate、PBT・差分・保存caseの再生         |
| Go         | `testing`                     | CGOなしのWasm adapter、有限モデル・実装・履歴の照合            |

共通にしたいのはモデル・値・観測・反例と検査の意味。実装への操作や埋め込み注釈の読み取りは、各言語のDriver/frontendが担当する。各言語の全APIがそろっている段階ではない。

## 現在の実装とこれから

| 項目                         | 現在地                                                                                  |
| ---------------------------- | --------------------------------------------------------------------------------------- |
| モデルと実装の照合           | 有限モデル、Driver、共通オラクル、履歴照合と再生を実装。TS/Rust/Goの接続例がある。      |
| 生成・縮小・障害や順序の制御 | PBT・差分テスト・保存caseの再生、明示した時計・通信・永続化・actor順序を扱う。          |
| TLA+系のモデル検査           | Rust/Wasmのparser・compiler・checkerとtla-ts経路。対応範囲はサブセット。                |
| SMT                          | 公開TS adapterはZ3 Wasmを利用。独立した`rusmt`は対応サブセットをnative/Wasmで解く試作。 |
| TLA+とSMTの相互運用          | 共通backend契約はあるが、値・式の変換と統合した検査経路は今後の対象。                   |
| 埋め込みinvariant            | モデル状態のinvariant検査はある。各言語のソース注釈の抽出・VC生成は未実装。             |

`idealis tlc -workers 1 -config Spec.cfg Spec.tla` と `idealis tla Spec.tla` は Java なしで実行できる。[対応オプションと終了コード](crates/idealis-cli/README.md#tla-ファイルを検査する)を参照。npm の CLI は `idealis`、Rust の JSONL 実行口は `idealis-cli`。npm packageは`@mizchi/idealis`。[旧名からの移行と保存形式の互換性](docs/migration.md)を参照。Quint Connectの接続方法とWhy3のfrontend/backend分離を参考にしており、両ツールの言語・APIとの互換実装を提供する段階ではない。

## 構成とライブラリの役割

各crateの[対応チェックリストと任意の Jev レビュー](docs/crate-checklists.md)を参照。

roracleのrequest/report型はRustからTSへ生成する。Node向けnapi-rsは[比較実験](evals/experiments/roracle-native/README.md)として追加し、既定のWasm経路を維持する。他crate全体の型生成やnative bindingは未対応。

| ライブラリ                                  | 入力                                           | 担当する処理・出力                                                                         |
| ------------------------------------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------------ |
| [`rtla`](crates/rtla)                       | TLA+ソースとTLC設定                            | 構文解析しASTを返す。名前解決とモデル検査は後段が担当する。                                |
| [`rtla-compiler`](crates/rtla-compiler)     | rtlaのAST                                      | 名前解決、定数・operatorの展開、対応する有限モデルのIRへの変換。                           |
| [`rtlc`](crates/rtlc)                       | 検査用IR                                       | 状態列挙、invariant・deadlock・temporal property・fairness検査と反例。nativeの探索並列化。 |
| [`idealis-core`](crates/idealis-core)       | TLA+ソース、IR、有限モデル、観測・履歴         | frontendとcheckerの接続、実装照合、履歴検査、再生とnative JSONL/Wasm境界。                 |
| [`rusmt`](crates/rusmt)（旧idealis-smt）    | 対応するSMT-LIB式                              | 任意精度Intの差分制約とBool式。SATモデル、UNSAT、work切れのunknownを返す独立した試作。     |
| [`ralloy`](crates/ralloy)                   | 静的なAlloyソースと有限scope                   | sig・二項関係・fact・assertの解析と、有限構造の列挙・instanceや反例の生成。                |
| [`idealis-cli`](crates/idealis-cli)         | JSONL の要求と実装の観測                       | idealis-connect のセッションを起動し、生成した操作列とモデル差分の判定を返す。             |
| [`idealis-connect`](crates/idealis-connect) | 有限生成器、保存case、feedback、複数実装の観測 | 生成・縮小・差分比較・再生をRustで管理し、各言語のDriverへ評価を要求する。                 |
| [`roracle`](crates/roracle)                 | 固定入力・独立したreference・実装callback      | 同じ登録で差分テスト・保存oracle再生・正しさを確認した比較ベンチを実行する。               |
| [`@mizchi/idealis`](packages/idealis)       | TSのモデルとDriver、backendの入出力            | オラクル生成、適合性照合、差分・PBT、反例縮小、スケジューリングとCLI。                     |
| [Go adapter](adapters/go)                   | 共通コアのWasmとGoのモデル・実装・履歴         | CGOなしでRustコアへ接続し、同じ値・オラクル・観測で照合と再生を行う。                      |

RustのTLA+経路は、`TLA+ソース → rtlaのAST → rtla-compilerのIR → rtlcの探索・検査`。`idealis-core`がこの経路をnative/Wasmのリクエストへ接続する。TSはRust frontend APIから利用し、Go adapterは共通コアの有限モデル・履歴照合を利用する。rtlaとrtlcはそれぞれ単独で利用でき、idealis-coreへ依存しない。

rtlaは固定SANYの全operator表とUnicode alias、LAMBDA/RECURSIVE、引数付き・LET内INSTANCE、nested module、一般selectorと構造付きproofを解析する。`syntaxVersion`で1.7.4/1.8.0の優先順位を選ぶ。Rust compilerは高階operatorを特殊化し、明示IR v2で再帰expressionと代入actionを予算付きで実行する。[対応チェックリスト](crates/rtla/SUPPORT.md)、[独立SANY/TLC比較](evals/iterations/0070-tla-coverage.md)と[frontendの契約](docs/rust-frontend.md)を参照。構文受理と有限モデルの検査、THEOREMの証明は別の範囲である。

Rust frontendは未使用定義を含むsource level制約を検査する。明示IR v2では、alias/actual経由の有限関数を選択とDOMAINで遅延評価し、frameを必要とする非再帰actionとtemporal helperのbodyを共有する。TS frontendも非再帰actionを共有する。全SANYの名前・型・proof検査と全観測位置でのTLC遅延評価互換は対象外。[範囲と再現手順](docs/frontend-semantics.md)を参照。

共有化の前後を同じ7入力・workで比較した。Apple M5上のCriterion、各4 runの平均の中央値。単位µs。

| 反復するbody    |    compile 前 → 後 | compile倍率 | prepared check 前 → 後 | prepared倍率 |
| --------------- | -----------------: | ----------: | ---------------------: | -----------: |
| action、深さ6   | 3181.000 → 220.402 |      14.43× |       108.445 → 90.971 |        1.19× |
| temporal、深さ5 | 2548.725 → 795.399 |       3.20× |      306.134 → 257.879 |        1.19× |

起動・IPCとTLCの速度は含めない。反復bodyを持つ人工モデルの測定で、全モデルの高速化を保証しない。[全21項目・条件・profile](evals/iterations/0072-frontend-semantics.md)を参照。

[QuintがApalache経由で生成したTLA+の実行確認](docs/quint-generated.md)は、6モデル・12件の正常/違反設定をnative/WasmとTLCへ照合する。`:=`、fold、有限選択、VariantsとCASEを含む限定したfragmentで、Quint言語やQuint Connect API全体の互換実装ではない。

`rusmt`はこのTLA+探索経路から独立している。現在の公開SMT adapterはZ3 Wasmを使い、rusmtを公開backendへ組み込む作業は今後の対象。

## 対応範囲と統合状況

TLA+・SMT-LIB・Alloyの対応はサブセットに限られる。モデル検査が確認するのは、指定したモデル・仮定・探索範囲に対する性質。実装との照合は、実行した操作列と観測を対象にする。予算内で反例が見つからなかった結果や、実装との一致を、全入力・全実行の正しさの証明へ読み替えない。[検証範囲](#検証の範囲)と各crateの制約を確認する。

Rust/Wasm frontendは有限domain上の量化fairness、SelectSeq / SortSeq / Assertを扱う。callbackや探索範囲の制約は[対応チェックリスト](crates/rtla-compiler/SUPPORT.md)に記録している。

TLA+はRust/Wasm frontendとtla-ts経路を持ち、`z3-solver`を任意のSMT adapterとして利用する。Quint Connectのモデルと実装をつなぐアプローチを参考に、TSの`Driver`を実装との境界にした。Quint言語やApalacheの再実装ではない。

[独立した`rusmt` crate](crates/rusmt)は、整数の差分制約とBoolean式をnative/Wasmで解く。定数倍、整数`ite`、Rust/JSONのassert・push/pop・一時Bool assumptionsに対応する。SMT-LIB scriptのpush/pop・複数checkも、冒頭で宣言を固定する範囲で実行できる。[対応チェックリスト](docs/smt-checklist.md)と[SMT-LIBサブセット](docs/smt-subset.md)を明示し、idealisの他crateやZ3には依存しない。Z3を再利用した比較でも小さいTTL・amount・schedulerはnativeで約3.0-3.4倍速い一方、Boolean UNSATには後退と予算切れが残る。[全17入力の比較](evals/iterations/0031-smt-reuse.md)は切り出し前の測定で、idealisの公開backendへの組み込みは保留している。

[`ralloy`](crates/ralloy)は、静的なsig・二項field・関係演算・量化・run/checkを有限scope内で検査する独立crate。JVMやZ3に依存せず、nativeとホストimportのないWasmで同じ探索器を使う。Alloy AnalyzerのSAT変換、継承、時間・var、整数演算は未対応。`@mizchi/idealis/alloy`からWasmを読み、モデルの探索と実装の関係スナップショットの照合に使える。[利用手順](docs/alloy.md)と[評価手順](evals/experiments/alloy-subset/README.md)を参照。汎用backend契約とCLIへの接続は今後の対象。

[`idealis-connect`](crates/idealis-connect)は、有限状態機械のPBT・縮小・差分比較・保存caseの再生を行う独立crate。TSは`@mizchi/idealis/testing`から同じWasmを使い、Vitest・node:test・ブラウザでDriverを動かす。[明示したactor列からのスケジュール生成](docs/testing.md#actorの操作順を制御する)も扱う。既存fast-check経路のStrategy/seed互換、任意threadの自動計装、全実行の証明は含めない。

[`roracle`](crates/roracle)は、Rustの`#[test]`からcaseと複数実装を登録し、差分検査と比較ベンチで共有する。保存したオラクルへの再生と、測定中の結果変化の検出を扱う。PBT・縮小はidealis-connectと併用する。

開発用の[構造付きfuzzing](evals/experiments/structured-fuzz/README.md)では、testing protocolと保存caseをASanで検査し、保存corpusをnative/Wasmで再生する。到達性、状態破損の対照、実行予算と採用範囲は[0039の評価](evals/iterations/0039-structured-fuzz.md)に記録した。

[SAT/CNF候補の評価](evals/iterations/0037-sat-foundation.md)では、Varisat 0.2.2のnative/Wasmを独立全列挙と照合し、実ブラウザの動作を確認した。実行バジェットを保証するbackendへの採用は保留中。`ralloy`や`rusmt`には組み込まず、非公開のeval crateとして分離している。

`@mizchi/idealis/patterns` は、ブラウザ履歴・Node.js のコールバック順序・オブジェクトストアを、同梱の Idealis モデル と照合する接続パターンを提供する。保存した観測は ITF にして再照合できる。[使い方](docs/model-patterns.md)

[はじめての Idealis](docs/ideal-tutorial.md) では、図書館の貸出を `.ideal` で書き、不変条件・到達性・返却を待つ性質を検査する。

## 実行

観測付きの有限モデルを TS / Rust で共有する方法、反例の縮小、時計・通信・永続化の注入は [利用ガイド](docs/using-models.md) を参照。`just package` で solver を必須にしない npm/Cargo/Wasm archive と CLI を作り、別 consumer で検証できる。[公開手順と CI の分割](docs/releasing.md) は別途記載している。

Node.js 24+、pnpm 12+、just を使う。リポジトリで Rust backend とWasmの開発検査を実行する場合は Rust と `wasm32-unknown-unknown` target も必要。npm の TLA+ CLI をビルドする場合は `wasm32-wasip1` も使う。配布 Wasm の利用側で Rust をビルドする必要はない。npm には未公開。

```sh
just install
just check
just demo
just mutation
rustup target add wasm32-unknown-unknown wasm32-wasip1
just wasm-test
```

pnpm workspace のルートで開発タスクを実行する。公開パッケージは [packages/idealis](packages/idealis) にあり、SDK と CLI の TypeScript 部分は `just build-sdk`、配布する Wasm を含む CLI は `just package-prepack` でビルドできる。

`just check` は Vite+ の format・lint 検査、TypeScript の型検査、Vitest、`node:test` を実行する。テスト対象と lint・format のルールは `vite.config.ts` にまとめている。単独で lint を実行するには `just lint`、整形を書き込むには `just fmt`、全 Vitest プロジェクトを実行するには `just vitest` を使う。`just wasm-test` は実Wasmの判定・ABI・反例再生・エラー復旧とnativeとの照合をNodeで実行する。ブラウザやDOMの起動は必要ない。共通の挙動テストを両ランナーに登録している。`just mutation` は `@mizchi/mutator` で TS ソースを変異させ、Vitest で検査する。

評価と改善のループは `just eval-loop`、保存した故障の再現は `just replay-loop`。[evals/](evals/README.md) に仮説・条件・run・判断・次の仮説を残す。初回の幅広い比較は `just evaluate` / `just replay-evaluation` で再実行できる。実験用の探索器・履歴 checker は public API に含めない。

時間・メモリから worker 数と探索深さを選ぶ `./budget`、独立 task の分割・集約を行う `./sharding` を追加した。[実行時間・Rust・cloud sharding の評価](evals/experiments/runtime/README.md) と [測定結果](evals/iterations/0003-runtime.md) を参照。Rust/Wasm は実験用カーネルとして分離している。

独立した検査の配置と常駐workerへの配布は `./scheduling`。[予算・費用順配置・batch化のガイド](docs/scheduling.md) と [TLA検査の1/2/4/8-worker評価](evals/iterations/0017-parallel-safety.md) を参照。共通APIはNodeやsolverに依存せず、Node / browserのTLA adapterは評価用の試作として分離している。

## operatorを共有するIR v2

`compileTlaSource(core, { source, config, irVersion: 2 })`またはTS frontendの`compileTlaSafety({ source, config, irVersion: 2 })`で、operatorの式定義を共有できる。既定はv1。引数はcallerの字句環境を保持し、要求時のcurrent/next/primeで評価する。未使用引数を先に実行せず、callと引数要求はworkへ課金する。

Rustのv2はmodule連結と設定置換にも対応する。SYMMETRYはv2では未対応。明示v2では再帰するInit/Nextの代入actionを共有定義へ変換する。TS frontendとRust frontendの受理範囲も異なる。公開`TCommit`のRM=8はRustのv1/v2とTLCで6,816状態・53,513生成が一致し、Quintの6生成物・正常/故障12設定も検査する。これらは有限scopeでの照合で、THEOREMの証明ではない。[IR契約と制限](docs/safety-ir-v2.md)を参照。

| 固定したsharing chain  |                v1 | 明示v2 |
| ---------------------- | ----------------: | -----: |
| 深さ10のIR bytes       |           330,878 |  4,707 |
| 深さ10のcompile要求 ms |            35.534 |  0.359 |
| 深さ60のIR bytes       | nesting上限で拒否 | 24,557 |

compileは常駐native JSONLの要求・IR応答を含む中央値。未選択branchのinlineを除く効果で、指数的な評価仕事をなくしたものではない。公開TCommitの新規Rust processは統合前114.85 ms、統合後v1が124.05 ms、v2が129.32 msで、実行費用の増加も残る。[費用・profile・採否](evals/iterations/0062-p0-checker.md)を参照。

## 同じSafetyProgramの反復検査

`prepareSafetyProgram(core, program)`で同じIRの検証と定数索引を再利用し、`check`と`checkBatch`へ異なる探索予算・optionsを渡せる。各jobの探索状態とworkは独立する。batchは1〜256件の結果またはerrorを位置を保って返し、`release()`で破棄する。Rustは`program.prepare()`が同じ契約を提供する。

Counterの100 queryでは、準備・破棄込みのnative v1 API時間がone-shot 3.883→batch 0.643 ms、予算を変えるqueryが2.162→0.256 msだった。探索主体のTCommitでは効果が小さく、1件だけの検査にも同じ倍率は当てはまらない。[APIと予算の扱い](docs/prepared-safety.md)、[条件・cold/RSS・後退の確認](evals/iterations/0064-prepared-safety.md)を参照。

過去の評価でのprepared探索は、TLA+コンパイル済みIRのone-shot検査を指す。

## node関数に限定したSYMMETRY

Rust frontendで、`Symm == Permutations(Nodes)`と単一のuntyped model value集合を指定できる。全Nodesをdomainに持つInt/Bool関数の値をnodeごとの行へまとめ、複数関数の相関と具体的な反例を保って状態を同一視する。3ノード・3値のcounterは27状態から10状態へ縮約した。7ノード・6値では279,936→792状態となり、Rust逐次の起動込み中央値は673.5→36.5 ms（約18.5倍）、prepared探索は633.2→3.89 ms（約163倍）だった。この限定したモデルでの結果で、全TLA+への速度保証ではない。

宣言がInit/Next/invariantを保つことは利用者の前提。自動的な対称性の証明ではない。対象はsafetyのみで、node参照payloadやtemporal/fairness等との併用を拒否する。[設定例と条件](docs/symmetry.md)、上記数値を測った[0060の評価](evals/iterations/0060-restricted-symmetry.md)を参照。高速化の採否はRust nativeを主指標にし、Wasmは可搬性と挙動一致を保つ。

0061ではnodeごとのflat Int/Bool recordとnode部分集合へ拡張した。`symmetryActions`で同じnode行の遷移を代表だけ生成し、nativeは64状態以上のfrontierで並列展開できる。最終版の確認測定ではrecord N5が縮約なし20.69→縮約あり2.12→代表展開1.60 ms、record N12の代表展開は1→8 workerで92.6→80.9 msだった。いずれもprepared探索で、起動込みの速度比ではない。反復間の大きな変動と再測定も保存している。

独立したBool flagの一度だけの設定操作には、opt-inの限定PORも使える。10 flagの人工モデルでは4,096→14状態、prepared探索3.74→0.0105 msだった。一般のPORへ適用範囲を広げず、元のグラフでの最短反例も保証しない。命令IDの直接実行はcounterで約40%後退したため不採用。[実行optionと制限](docs/reductions.md)、[0061の採否と測定](evals/iterations/0061-symmetry-next.md)を参照。

起動とcompiled IRのJSON境界を含む別測定では、record N12の代表展開は99.3→84.6 ms、ピークRSSの中央値は12.8→15.2 MiB（1→8 worker）だった。Node計測driverとtime wrapperの費用を含み、Node hostの起動とTLA+ frontendは含まない。小さいPORモデルのwallは6.89→3.70 msで、prepared探索の倍率とは分ける。

0065ではnode行を連続配置し、現在stateのOption列コピーも省いた。変更前後の固定binaryを交互に測る21本のCriterion比較で、SYMMETRYの検査時間を約8〜17%短縮した。record N12の代表展開は1 workerで85.57→71.80 ms、8 workersで73.01→64.41 ms。縮約条件・生成数・論理workは変えず、以前の測定値とは別に比較する。[条件・対照・採否](evals/iterations/0065-symmetry-allocation.md)を参照。

0067では検査済みの先頭recordと同じ内部値に限り、schemaの再構築を省いた。21本の同条件比較で、SYMのrecord 6本が約6.5〜9%短縮した。record N12の代表展開はRustのIR検査で73.40→67.95 ms（1 worker）、63.89→59.49 ms（8 workers）。source compileとIR checkの常駐API区間では77.52→75.51 / 67.22→63.64 msだった。論理workと異なるrecordの検査を維持し、Arenaをまたぐcacheは持たない。[全sample・変動・採否と再開条件](evals/iterations/0067-record-schema.md)を参照。

今回のmicro tuningはprepare/batch、行配置、Ref lookup、同一recordのschema再検査までで区切る。shape cacheやborrowed entriesは新しいprofileで支配費用を確認し、小整数圧縮は既存0/1圧縮で足りない実モデルのRSS制約、worker制御は広いfrontierでの待機・merge費用を示せたときに再開する。[残る候補と再開条件](https://github.com/mizchi/idealis/issues/63)を参照。

## Data定数のprepared化

0059ではData定数の初回成功結果をslotへ保持し、繰り返しの値全体のhashを省いた。以下は約210万状態のチャネル群を全探索した同じrunの起動込み秒、1 warmup後3回の中央値。

| 実行方式           | 変更前 | 変更後 | 前/後 |
| ------------------ | -----: | -----: | ----: |
| native 1 worker    | 14.075 | 13.463 |  1.05 |
| native 8 workers   |  5.887 |  5.432 |  1.08 |
| Node Wasm 1 worker | 21.191 | 19.308 |  1.10 |

native 8は反復の幅が重なり、最終反復では逆転した。関数モデルのnativeはほぼ横ばい。チャネルのWasmは同じrunのtla-ts（16.374秒）より約18%遅い。型別命令と並列の訪問済み事前照合も順に試したが、後退があり本体への採用を見送った。[9経路の全sample・14本の段階別比較・採否](evals/iterations/0059-prepared-execution.md)を参照。

## packed関数のhash最適化

0058では0/1整数関数N20を完全探索した。共通TS frontendを含む起動込み秒、1 warmup後3回の中央値。Apple M5 arm64、Node 24.22.2、tla-ts 0.0.10で測定した。状態数は1,048,576、生成数は20,971,521で全経路が一致した。

| 実行方式           | 変更前 | 変更後 | 前/後 |
| ------------------ | -----: | -----: | ----: |
| native 1 worker    |  3.377 |  3.191 |  1.06 |
| native 8 workers   |  2.399 |  2.280 |  1.05 |
| Node Wasm 1 worker |  5.017 |  4.548 |  1.10 |

このfunction条件では、native 8 workersはTLC 8 workers比1.84倍、Wasmはtla-ts比1.12倍だった。チャネル側は参照実装も大きく変動し、今回は速度改善の根拠へ使わない。Criterionのring逐次は所要時間が約8%増えた。[全sample・対照・互換性検査](evals/iterations/0058-packed-hash.md)を参照。Node Wasmの結果はブラウザの速度ではない。

## 0057時点の関数更新の比較

0057の同じrunで、tla-ts・TLC・変更後のidealisを完全探索した。起動と共通TS frontendを含む秒数で、1 warmup後3回の中央値。状態数・生成数は全経路で一致した。

| モデル      | tla-ts 1 |  TLC 1 | TLC 8 | idealis Wasm 1 | idealis native 1 | idealis native 8 |
| ----------- | -------: | -----: | ----: | -------------: | ---------------: | ---------------: |
| 0/1整数関数 |    5.648 | 13.487 | 4.391 |          5.595 |            3.910 |            2.609 |
| チャネル群  |   15.829 | 23.260 | 6.341 |         20.541 |           13.670 |            5.531 |

0057の変更前比でチャネルのnative並列探索は1.11倍。逐次とWasmの差は小さく、関数モデルは約1〜2%後退した。チャネルのWasmはtla-tsより約30%遅い。全モデルが速くなる変更とは扱わない。

Apple M5 arm64、Node 24.22.2、tla-ts 0.0.10、固定arm64 Java/TLCを使用した。WasmはNode上の値。非専用ホストの負荷変動を含む。[全sample・最小/最大・Criterion・互換性検査](evals/iterations/0057-probe-updates.md)を参照。domain共有とbit圧縮の[前回0056](evals/iterations/0056-compact-functions.md)、下の0022時点の4モデル表は別runで、今回の値と混在させていない。

同一入力の完全探索結果を再利用する`./check-cache`は、ブラウザでも動くTSコアとSQLite/HTTP adapterを持つ。[再利用条件・所有権・世代番号をLeanで形式化](docs/check-cache.md)し、有限差分テストと実ブラウザで実装を検査する。Rust/Wasmには[状態ごとのNext評価の再利用](docs/safety-expansions.md)も任意で追加し、現在のInit・property・constraintsを再検査する。状態cacheは既定無効で、`enabled: true`を明示して有効にする。Leanは開発時だけ必要。状態探索自体の分散BFSとqueueのcheckpoint再開は今後の対象。

<!-- checker-comparison:start -->

## モデル検査の速度比較

0062の同一runで、同じTLA+ source/configを各回別processから完全探索した時間。単位は秒、各条件1 warmup後3回の中央値。Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、tla2tools v1.7.4 / Temurin 21.0.13+9。native/Wasmは共通のTS frontend・Node起動・IR転送を含むIR v1経路で、Rustのsource APIだけの時間とは分ける。

| モデル      |    状態数 | tla-ts 1 worker | TLC 1 worker | TLC 8 workers | idealis Wasm 1 worker | idealis native 1 worker | idealis native 8 workers |
| ----------- | --------: | --------------: | -----------: | ------------: | --------------------: | ----------------------: | -----------------------: |
| カウンタ群  | 5,308,416 |           6.490 |        6.913 |         7.922 |                 4.195 |                   3.218 |                    2.968 |
| Boolean関数 | 1,048,576 |           5.424 |       12.781 |         4.325 |                 4.613 |                   3.485 |                    2.440 |
| チャネル群  | 2,097,152 |          15.728 |       23.247 |         6.285 |                19.977 |                  13.809 |                    5.497 |
| 幅1のring   | 1,000,000 |           0.407 |        0.926 |         1.474 |                 0.255 |                   0.182 |                    0.181 |

Wasm列はNode上の値。worker数は要求上限で、nativeの幅1モデルはpoolを起動しない。tla-tsは1 workerのみを測った。TLCとNodeのheap設定は8 GiBで、共通の厳密なRSS上限ではない。非専用ホストの負荷変動を含む。対応する有限fragmentの測定で、モデル全般の速度を保証しない。

nativeの8-worker要求は同じrunのTLC 8 workers比でcounter 2.67倍、function 1.77倍、channel 1.14倍。共有IR統合前との比較ではfunctionのnative 1/8が約8%/6%遅くなった。channelのnative 8は速くなったが、変更前の反復間の幅も大きい。Wasmのchannelは同じrunのtla-tsより約27%遅い。

[全sample・最小/最大・profileと条件](evals/iterations/0062-p0-checker.md)を保存する。旧[0022の4モデル比較](evals/iterations/0022-readme-comparison.md)と同じ表へ値を混ぜない。常駐APIやIR v2の費用は別に記録する。

<!-- checker-comparison:end -->

0063ではData定数のslot検索を調整し、同じrunのTCommit（RM=8）のnative IR検査要求がv1で143.77→113.24 ms、v2で140.55→118.32 msになった。起動込みは138.70→118.61 ms / 143.87→125.93 ms。各1 warmup + 3 samples、非専用M5での中央値で、IR decode・prepared構築を含む。14本のkernelには数%の後退も残る。

[採用したhash変更・撤去したinline・全sample](evals/iterations/0063-evaluator-tuning.md)を参照。上の0062のTLC比較とは別runの値。

0066ではRef domainのlookupで候補添字を完全key一致後に使い、外れたら線形検索へ戻す。固定binaryのTCommit IR検査要求を各12回測ると、native v1は105.01→97.61 ms、v2は109.61→102.55 msだった。初回の3 samplesとcoldでは約3〜4%の短縮で、条件の異なる値を混ぜない。通常14本には最大約6%の後退も残る。[条件・対照・採否](evals/iterations/0066-domain-offset.md)を参照。

## 同じ登録による差分検査と比較ベンチ

新しい`roracle`で、TLC・tla-ts・idealisへ同じ小さいモデルを渡した。下表は起動込みのcold source-to-result中央値、7 sample・1 warmup、すべて1 worker。process起動・輸送・解析・raw log書き込みを含む。

| モデル        | idealis native (ms) | tla-ts (ms) | TLC (ms) |
| ------------- | ------------------: | ----------: | -------: |
| linear-ring   |               4.505 |      71.607 |  584.650 |
| function-cube |               2.675 |      78.875 |  593.217 |

ringは1,000状態、function-cubeは8 bitの256状態で、3実装の完了判定・distinct・generatedが一致した。小さいモデルの起動費用を含み、検査kernelの速度差を示す値ではない。Rustのprepared check/coverage比較、測定中のdrift検出、保存oracleからの再生と最初の失敗は[評価0051](evals/iterations/0051-oracle-library.md)へ記録した。APIは[RustのテストとCriterion風のgroup登録](crates/roracle/README.md)を参照。

workspaceの差分検査は23ファイル・99比較を同じRust/Wasm照合器へ移した。変更前後のbinary/source snapshot、cold/reusedの比較、保存oracleへの再生を[チューニング手順](evals/experiments/workspace-oracle/README.md)で実行できる。正常対照の12 group・1222回の一致と、JSON bridgeの追加費用は[評価0052](evals/iterations/0052-workspace-oracle.md)へ記録した。 TLCの参照環境を修正し、既存24不一致も[評価0053](evals/iterations/0053-tlc-standard-modules.md)で解消した。155件の検査・32件の確認・155件の保存再生で不一致0を確認している。

## Rustコアの最適化前後

上の6経路比較は最適化前の0022時点の記録。今回の6項目の最適化は、変更前後のnative/Wasmを別途交互に測った。起動込み秒、1 warmup後3回の中央値。比率は変更前/変更後。TLC/tla-tsの過去値はこの表へ混ぜていない。

| モデル      | native 1: 変更前 → 後 (秒) |  比率 | native 最大8: 変更前 → 後 (秒) |  比率 | Wasm 1: 変更前 → 後 (秒) |  比率 |
| ----------- | -------------------------: | ----: | -----------------------------: | ----: | -----------------------: | ----: |
| カウンタ群  |              3.114 → 3.061 | 1.02× |                  3.781 → 2.996 | 1.26× |            3.944 → 4.822 | 0.82× |
| Boolean関数 |              9.609 → 7.221 | 1.33× |                  5.407 → 3.942 | 1.37× |          19.809 → 10.585 | 1.87× |
| チャネル群  |            22.260 → 17.838 | 1.25× |                 13.500 → 8.829 | 1.53× |          38.771 → 24.964 | 1.55× |
| 幅1のring   |              0.213 → 0.215 | 0.99× |                  0.223 → 0.217 | 1.03× |            0.268 → 0.286 | 0.94× |

最大8のcounterは軽いscalar演算と判定して逐次実行する。Wasmのcounter/ringは後退が残る。Boolean関数のnative RSSは約40%、チャネル群は約35%減った（200 ms間隔の観測）。[Criterion・全sample・範囲・採否と再実行](evals/iterations/0023-criterion-safety.md)に記録した。`just criterion-test`でsmoke、`just criterion-save <stage> <新規runパス>`で14本を測る。

## CPU profileを使った追加調整

0023後の版から、scalarのleaf評価と整数keyの関数検索を調整した。以下はprofilerを外した通常Wasmの新しい交互比較で、起動込み中央値。上の表とは別run。

| モデル     |     変更前 → 後 | before/after |
| ---------- | --------------: | -----------: |
| カウンタ群 | 4.476 → 4.136 s |        1.08× |
| 幅1のring  |    272 → 255 ms |        1.07× |

native Criterionのcounterは12.285→9.266 ms、functionは11.071→9.416 msだった。整数位置の変更単独はchannels最大8で後退する回もあり、全ケースの高速化を保証する結果ではない。[profile・14本の段階別測定・16 sampleと再生](evals/iterations/0024-profile-tuning.md)を保存した。`just profile-test`、`just profile-criterion <stage> <新規runパス>`で確認できる。

## SMTスクリプト解析の最適化

同じrusmtの改善前後をCriterionで比較したnativeの解析時間。Apple M5、20 samplesの中央値で、JSONL/Wasmや求解の時間は含めない。全14ケース、ばらついた測定と再確認の条件は[評価0042](evals/iterations/0042-smt-tuning.md)を参照。

| scriptの内容                 |   改善前 |   改善後 |  倍率 |
| ---------------------------- | -------: | -------: | ----: |
| 64 check、整数チェーン32変数 | 4.701 ms | 1.183 ms | 4.0倍 |
| scope操作32組、64 check      | 415.2 µs | 239.0 µs | 1.7倍 |
| 整数ite、64 check            | 212.7 µs |  31.8 µs | 6.7倍 |

## モデルと実装の比較

TS / Rust の共通コアは `./portable`。同じ Rust ソースを native と Wasm で使い、適合性照合とトレース再生を行う。[Go adapter](docs/go-adapter.md) は CGO 不要で同じ Wasm に接続する。[値・effect protocol・使用例](docs/portable-core.md) を参照。`just portable-build` / `just portable-test` で Rust と Node・実ブラウザ、`just go-test` で Go の照合と単独 consumer を検査する。

[共通オラクルで3言語の実装を動かす評価](docs/shared-oracle.md) は `just cross-test`。同じ有限モデルと保存 trace を使い、正常版・故障版の TS / Rust / Go を直接操作して比較する。

[非同期履歴とモデル再利用](docs/async-history.md) は `./history` と `loadFiniteModel`。未完了・結果不明の操作を含む照合、索引付き handle、依存関係を使う縮小と探索 feedback の評価は `just async-test` から実行できる。

[TLA+ の Rust backend](docs/rust-safety.md) は `compileTlaSafety` と `checkSafety`。整数・集合・関数・record・sequence の有限モデル、temporal property、WF / SF を IR に変換し、状態生成・BFS・SCC 検査・反例復元を native / Wasm 内で実行する。全 TLA+ 互換ではなく、対応外のモデルは既存の tla-ts を使う。

検査器は独立した[`rtlc` crate](crates/rtlc)へ分離した。IR・値評価・探索・temporal/fairness・native並列化を持ち、idealis-coreへの依存はない。idealis-coreは実装照合・履歴・host・縮小と既存Wasm/JSONL adapterを担当する。`just rtlc-test` / `just rtlc-example`で単独利用を検査し、`just package`で両crateと単独consumerを確認できる。

構文解析は独立した[`rtla` crate](crates/rtla)、ASTからIRへの変換は[`rtla-compiler`](crates/rtla-compiler)が担当する。[Rust frontend](docs/rust-frontend.md)の`compileTlaSource` / `checkTlaSource`で、ブラウザでもtla-tsなしでソースの解析・変換・検査を実行できる。既存の`compileTlaSafety`はtla-tsを使う比較対象として残す。`just compiler-test` / `just compiler-eval <新規run>`で検査し、`just package`で9つのcrateとWasmの配布物を確認できる。

[Apalacheとの比較](docs/apalache-comparison.md)は、SMTによる有界検査とrtlcの状態列挙の保証を分け、同じ6条件の起動込み時間と反例を記録する。小規模pilotを大規模モデルの速度順位へ一般化しない。

[`@mizchi/idealis/backend`](docs/backend-contract.md)は、完全探索、有界の反例なし、帰納性を区別する共通契約。model/property/意味論/仮定/backend版を結果へ結びつけ、空の初期集合・資源切れ・unknownを検査成功へ変換しない。rtlcのnative/Wasm bridgeを提供する。Apalacheの汎用adapterとSMT encodingは今後の対象。[契約と保存応答の検査](evals/iterations/0028-backend-contract.md)に結果を記録した。

```ts
import { checkConformance, checkModel, checkTrace } from "@mizchi/idealis";
import { counterModel, createCounter } from "./examples/durable-counter.js";

const result = await checkConformance(counterModel, createCounter(), [
  { type: "add", id: "request-1", amount: 2 },
  { type: "crash" },
  { type: "restart" },
  { type: "add", id: "request-1", amount: 2 },
]);
if (result.status !== "matched") throw new Error(JSON.stringify(result));

const oracle = checkModel(counterModel, { maxDepth: 4, maxStates: 500 });
for (const trace of oracle.traces) {
  const replay = await checkTrace(trace, createCounter());
  if (replay.status !== "matched") throw new Error(JSON.stringify(replay));
}
```

例は独立した immutable な参照モデルと mutable な実装で構成する。障害は `crash` / `restart`、ネットワークは `send` / `deliver` / `drop` の操作として明示する。配信インデックスを選ぶことで遅延・並べ替えを再現する。OS の実際の通信やディスク障害を注入する機能は含まない。

`Model.initial()` と `Model.step()` は許容する状態と観測の候補を返す。`Driver.reset()` と `Driver.step()` は実装を操作して観測を返す。非決定的なモデルでは、観測に合う候補状態を履歴全体で絞る。ドライバーの例外は呼び出し元に伝わり、`dispose()` は終了時に呼ばれる。

## Property-based testing と差分テスト

```ts
import assert from "node:assert/strict";
import { array, constantFrom } from "fast-check";
import { checkProperty } from "@mizchi/idealis/property";
import { checkDifferential } from "@mizchi/idealis";
import {
  counterModel,
  createCounter,
  type CounterAction,
} from "./examples/durable-counter.js";

const actions = array(
  constantFrom<CounterAction>(
    { type: "add", id: "a", amount: 1 },
    { type: "crash" },
    { type: "restart" },
  ),
  { maxLength: 24 },
);

const result = await checkProperty(counterModel, createCounter, actions, {
  seed: 42,
  numRuns: 100,
});
assert.equal(result.status, "passed");

const diff = await checkDifferential(createCounter(), createCounter(), [
  { type: "add", id: "a", amount: 1 },
]);
assert.equal(diff.status, "matched");
```

このコードは Vitest の `test()` 内でも `node:test` 内でも動く。`persist: false` の故障実装では、反例が `add → crash → restart` に縮小する。結果の `seed` と `path` を同じ arbitrary・fast-check バージョンとともに渡すと再実行できる。長期保存には操作列自体も残す。無効操作で全件が棄却される場合は `exhausted` になる。

デモは `.artifacts/oracle.json`、`tla-oracle.json`、`counterexample.json` を生成する。`decodeTrace` は action / observation の型ガードを要求する。トレースはバージョン付き JSON で、bigint・Set・Map はアダプターで明示的に符号化する。

## アダプターと移植性

| import                             | 役割                                                   | ランタイム                     |
| ---------------------------------- | ------------------------------------------------------ | ------------------------------ |
| `packages/idealis/src/index.ts`    | 有限モデルの BFS、合成、適合性比較、差分比較、トレース | 外部依存のない TS              |
| `packages/idealis/src/property.ts` | 生成、縮小、seed/path 再現                             | fast-check                     |
| `packages/idealis/src/tla.ts`      | 実際の TLA+ 検査結果と有限反例の変換                   | tla-ts                         |
| `packages/idealis/src/smt.ts`      | SMT-LIB の SAT/UNSAT/UNKNOWN と整数解の取り出し        | 呼び出し側で初期化した Z3 Wasm |

コアを import してもソルバーは読み込まれない。通常CIはNodeのWebAssemblyで23件の共有契約をNode test runnerとVitestから実行し、ブラウザ向けbundleも検査する。Web Worker輸送やcross-origin isolationなどの実ブラウザ互換性は、手動の[Browser compatibility workflow](.github/workflows/browser.yml)と`just browser`で確認する。Z3 のブラウザ実行には `z3-built.js` / `.wasm` の配信、SharedArrayBuffer、COOP/COEP ヘッダーが必要。

`tla-ts@0.0.10` は Node/Bun 向け参照を含むため、ブラウザで通常の import をそのままバンドルすると失敗する。この試作では Vite+ の `resolve.alias: { "node:zlib": "fflate" }`、`build.rolldownOptions.external: ["bun:jsc"]`、`define: { global: "globalThis" }` を使う。Bun 専用の動的 import はブラウザでは到達しない。TLA+ は既定の単一ワーカーで検査する。具体的な配信とビルドは [ブラウザテスト](tests/browser.spec.ts) を参照。

Z3 は `init()` の結果を `createSmtOracle(api)` に渡す。Node では利用後に `killThreads(api.em)` を呼ぶ。整数解を十進文字列で返し、無制限整数の精度を保つ。SMT-LIB の assertion を渡す API であり、任意の TS 関数を自動で記号実行する機能はない。

## 検証の範囲

- モデル検査の `complete` は、提供された `actions` と衝突のない `key` に対して到達グラフが閉じたことを表す。打ち切りは `bounded`、初期状態なしは `empty`。TS コアの検査対象は状態不変条件で、liveness / fairness は含まない。
- 生成トレースは到達状態・遷移の代表的な witness。実装の全履歴を網羅するテスト集合ではない。モデル側の状態が同じでも、実装には異なる隠れた状態があり得る。
- 適合性比較は指定した操作列・観測・抽象化に対する検査。`invalid` はモデルで実行できない操作、`mismatch` は許容観測からの逸脱。汎用の refinement proof や線形化可能性の証明ではない。
- `composeModels` は独立操作の interleaving と明示した `both` 操作の直積を探索する。モデル間の境界不変条件を指定できる。assume-guarantee 推論は行わない。
- TLA+ アダプターは成功時に witness を捏造しない。lasso の反例は有限オラクルへ変換せず、元の検査結果を残す。型や整数意味論を Z3 と自動で一致させる変換は含まない。

[Idealis 言語の設計](docs/language-architecture.md)、[実装照合の設計](docs/architecture.md)、[テスト手法の調査](docs/testing-research.md)、[検証記録](docs/validation.md) を参照。

再帰actionの呼出しは引数ごとに複製していた親frameを共有する。初期実装とのCriterion比較では、複数引数の準備済みcheckが1,036.8 µsから808.6 µsへ短縮した（約1.28倍）。捕捉変数の2ケースは約17〜19%短縮し、単一引数と分岐の準備済みcheckはほぼ同等だった。[対応範囲](docs/recursive-actions.md)と[測定条件・全結果](evals/iterations/0071-recursive-actions.md)を参照。TLCに対する速度比ではない。
