# idealis-connect

[対応範囲・非対応のチェックリスト](SUPPORT.md)を参照。

有限状態機械からの操作列生成、削除による縮小、保存caseの再生、差分比較を行うRustライブラリ。既定の構成はserde/serde_jsonだけに依存し、同じ実装をnativeとホストimport不要のWasmで使う。SUTのI/Oやテストランナーはホストが担当する。alpha版で、registryには未公開。

## セッションを駆動する

`Session::property(Plan)`を作り、`effect()`を読む。Evaluateはcorrelation ID、phase、caseを持つ。ホストが新しいSUTを初期化してcase.actionsを実行し、`resume(id, verdict)`へ評価を返す。Doneが最終結果。Rustの呼び出し例は[examples/counter.rs](examples/counter.rs)を参照。

Verdictはmatched、signature付きmismatch、invalid、reason付きunknown、message付きerror。mismatchを検出すると同じsignatureを保つ削除候補を検査し、最後に再現を確認する。unknownの候補がある場合、縮小をminimalと判定しない。異なるsignatureや非再現を成功へ変換しない。

結果はpassed、failed、exhausted、inconclusive、error、not-failing。passedは指定した数の有効な正常caseを実行した意味で、モデルの完全探索ではない。invalidだけならexhausted、work不足ならinconclusive。縮小のminimalは、生成器が受理する列の中で単一削除しても同じ失敗を保持できない意味。予算で打ち切った縮小はboundedになる。scalar値の縮小は含めない。

`Session::shrink(generator, case, max_checks, max_work)`は既知のcaseを縮小する。`Session::replay(case, expected_signature)`は保存したactionを使い、乱数から再生成しない。expected_signatureを指定した再生が失敗を再現しなければerrorになる。unknownや実行errorは別に保持する。

## 生成器と保存形式

Generatorはid/version、初期状態、各状態のaction/target、minLength/maxLengthを持つ。最初に目標長、初期状態、遷移をxorshift32-v1で選ぶ。seedは1-2^32-1。終端へ到達するとそこで止め、minLengthに届かない生成はskipする。縮小候補は同じ有限状態機械と長さの条件を満たす必要がある。汎用Proptestやfast-checkのStrategyとseedの互換性は提供しない。

Caseはformat=idealis.test-case、version=1、rng、generatorId/generatorVersion、seed/index、全actionsを持つ。モデルやDriverの版は評価側のmanifestへ固定する。JSONでは配列の順序を保持し、objectのfield順は比較に影響しない。数値はsafe integerに限り、任意精度整数・集合等はホストで既存のPortableValueへ正規化してから渡す。

上限は1024状態、10,000遷移、128 actions、action当たり8 KiB、JSON再帰32段、case/request当たり1 MiB。1セッションは最大256正常case、4096 attempts、512縮小checks、16 MiBのtrial記録を扱う。超える入力はprotocol/resource error。workは生成の各段階と候補の受理検査を数え、JSON復号・plan検査・SUT実行の時間を含めない。SUT評価回数はattempts/checksで制限する。

## 明示した並行操作列

`Schedule { id, version, actors }`の各actorは`ScheduleActor { id, operations }`。`Schedule::generator()`はprogram counterの直積からGeneratorを作る。actionは`{ actor, step, operation }`となり、同actor内の操作順を維持する。stepは0始まり。[examples/schedule.rs](examples/schedule.rs)を参照。

1-8 actor、合計128操作、最大1024状態を構築前に検査する。actor IDは一意にする。空の操作列も許す。最短長0、最長長は合計操作数で、prefixも生成・縮小の対象。任意の単一削除でactor内の順序やoperationが崩れる候補は受理しない。構築はSessionのwork予算外で、構造の上限により制限する。

SUTはホストが指定actorを1操作ずつ進める。threadやawaitの自動計装、ブロッキングやweak memoryのモデル化は含まない。PBTのpassedを全スケジュールの完全探索と解釈しない。Loomは評価crateだけで利用し、このcrateやWasmの依存へ含めない。

## 差分比較

`compare(Observation::Allowed { values }, targets)`は、referenceが許可する値集合と各実装の観測を比較する。対象を多数決でオラクルへしない。空の対象はerror、空の許可値集合はinvalid。invalid/unknown/errorはmatchedへ変換せず、対象ごとのoutcomeも残す。全体statusはerror、mismatch、unknown、invalid、matchedの優先順で決めるため、全体statusだけで他対象の判定が完了したとは扱わない。

## nativeとWasm

[idealis-cli](../idealis-cli/README.md)はこのライブラリのEngineを起動し、version=1のJSONLを扱う。requestのkindはschedule、property、shrink、replay、resume、cancel、compare。scheduleは`{ kind: "schedule", schedule: ... }`へ`{ kind: "generator", generator: ... }`を返す。property/shrink/replayは同時に1セッションだけ実行し、cancelで解放する。schedule/compareは進行中のセッションとは独立。無効なrequestやfeedback IDは進行中の要求を消費しない。

Wasm ABI v1はtesting_abi_version、testing_input_resize、testing_dispatch、testing_output_ptr、testing_output_len。ホストは同じJSONを渡す。並列に実行する場合はworkerごとに別runtimeを使う。

[Proptestの状態機械テスト](https://proptest-rs.github.io/proptest/proptest/state-machine.html)と既存TSの縮小器を参照した。外部solver、任意threadの自動計装、高階Strategy、scalar縮小はこのcrateへ組み込んでいない。

## Rust から TLA+ と実装を照合する

Cargo.toml に `idealis-connect = { git = "https://github.com/mizchi/idealis", branch = "main", features = ["tla"] }` を追加する。テストを固定するときは branch を導入時の rev に置き換える。[実行可能な HashMap の例](examples/tla_store.rs) は仕様と設定のパスを引数に取り、実装を操作して読み取った観測を使う。

```sh
cargo run --release --manifest-path crates/idealis-connect/Cargo.toml --features tla \
  --example tla_store -- HashMapModel.tla HashMapModel.cfg
```

`tla::finite_model(source, config, spec, bounds)` は完全探索した具体状態から `tla::FiniteModel` を作る。初期状態は探索の根から取得し、同じキーでも異なる初期観測を保存する。操作と観測は到達先の状態から読む。既定の変数名は `act` と `obs`。状態制約・操作制約で除外した辺、および時相検査が補った stuttering は含めない。予算切れ・empty・違反・対称性縮約ではエラーになる。

`ConnectSpec` で変数名を指定できる。既定では注釈変数を除いた値で状態をまとめ、各グループの全メンバーで（操作、射影先、観測）の集合が一致するときだけ採用する。拒否された場合は `project_annotations: false` で完全な状態を保持できる。Generator の 1,024 状態・10,000 辺を超える場合は、モデルの有限領域を小さくする。

手書きのテストアダプタに `tla::Driver` の reset / step を実装し、固定操作列には `tla::check_driver`、生成・縮小には `Generator::from_finite_model` と `tla::check` を使う。非決定的な観測後に操作を続けられない列は invalid として扱う。確認した mismatch は反例の操作列・失敗位置・許容観測・実測値を返す。

`report.counterexample_actions` は保存された反例の全操作を `Option<Vec<tla::Value>>` で返す。reset だけの反例は `Some(vec![])`、保存された反例がなければ `None` になる。確認できた不一致の実測値と許容観測は `report.counterexample` から取得する。
