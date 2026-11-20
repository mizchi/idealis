# idealis-connectの対応チェックリスト

チェック済みは有限生成器からの生成・評価・縮小・再生・差分比較。SUTとテストランナーはホストが所有する。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] 有限状態機械のGeneratorとxorshift32-v1のseedによる操作列生成。長さ・終端・受理条件を扱う。任意の tla feature は具体グラフから安全に射影し、有限モデルの生成・実 Driver の照合・縮小に接続する。
- [x] Session::propertyでEvaluate/Doneのeffectを返し、correlation ID付きの評価を受け取る。
- [x] matched、signature付きmismatch、invalid、unknown、errorを区別する。
- [x] 同じ失敗signatureを保つ操作削除と再現確認。受理条件を満たす単一削除に対するminimalと予算切れのboundedを区別する。
- [x] 保存Caseの全actionsを使う再生。seedからの再生成ではなく、期待signatureの非再現をerrorにする。
- [x] 許容観測集合と各targetの差分比較。対象の多数決をオラクルにしない。
- [x] Scheduleのactorごとの順序を保つ操作列と有限Generatorへの変換。
- [x] version付きCase、JSONのsafe integer、protocol/resource上限とwork予算。
- [x] serde/serde_jsonを使う独立Rustライブラリ、nativeのEngine::dispatchとホストimport不要のWasm ABI v1。

## 非対応・対象外

- [ ] scalar値の縮小、任意の高階Strategy、Proptest/fast-checkのStrategyやseedとの互換性。
- [ ] thread/awaitの自動計装、ブロッキング、weak memoryのモデル化、SUTの自動実行。
- [ ] 外部solverの内蔵、任意言語の静的検証、PBTのpassedからのモデル完全探索の証明。
- [ ] 一つのruntime内の並列セッション。workerごとに別runtimeを使う。

## 制約と根拠

- [x] passedは指定数の有効caseが正常だった意味。invalidだけならexhausted、work不足ならinconclusiveとする。
- [x] unknownの縮小候補がある場合にminimalとせず、異なるsignatureや非再現を成功へ変換しない。
- [x] Scheduleは1〜8 actor、合計128操作、最大1024状態。構築費用はSessionのwork予算外で、構造上限で制限する。

[README](README.md)、[Session](src/session.rs)、[縮小](src/shrink.rs)、[差分比較](src/compare.rs)、[Schedule](src/schedule.rs)、[評価](../../evals/iterations/0036-testing-core.md)を参照。
