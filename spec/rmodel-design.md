# rmodel の文法設計仕様

<!-- Generated from spec/rmodel-design.pkl by idealis-spec (plan 0092). Do not edit. -->

## 文法変更と遷移

### 追跡方法（tracking method）

発送時に選んだ Post または Courier。

- id: `shipment-carrier`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `Carrier`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `Post`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `Courier`

### 発送状態（shipment status）

未発送または、追跡方法を持つ発送済み。

- id: `shipment-status`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `status`

### 発送する（ship the order）

未発送の注文を、指定した追跡方法で発送済みにする。

- id: `ship-order`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `ship`

### 発送を確認する（acknowledge shipment）

注文の発送を確認済みにする。

- id: `acknowledge-shipment`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `acknowledge`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `acknowledged`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `Bool`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `true`
- モデル: `fixtures/rcheck/structured/shipped-transition.model` の `false`

### 採用判断（adoption decision）

変更を評価中にするか採用するかの状態。

- id: `adoption`
- モデル: `spec/models/rmodel-calibration.model` の `Decision`
- モデル: `spec/models/rmodel-calibration.model` の `Trial`
- モデル: `spec/models/rmodel-calibration.model` の `Adopted`
- モデル: `spec/models/rmodel-calibration.model` の `decision`

### 意味の証拠（semantic evidence）

対照との一致、差異、結果の欠落、空、探索未完了、未確認を記録する。

- id: `semantic-evidence`
- モデル: `spec/models/rmodel-calibration.model` の `SemanticEvidence`
- モデル: `spec/models/rmodel-calibration.model` の `Unchecked`
- モデル: `spec/models/rmodel-calibration.model` の `Equivalent`
- モデル: `spec/models/rmodel-calibration.model` の `Diverges`
- モデル: `spec/models/rmodel-calibration.model` の `EmptyCheck`
- モデル: `spec/models/rmodel-calibration.model` の `Incomplete`
- モデル: `spec/models/rmodel-calibration.model` の `MissingComparison`
- モデル: `spec/models/rmodel-calibration.model` の `semantics`

### ゴールの証拠（goal evidence）

事前の基準に照らした改善、改善なし、悪化、未測定を記録する。

- id: `goal-evidence`
- モデル: `spec/models/rmodel-calibration.model` の `GoalEvidence`
- モデル: `spec/models/rmodel-calibration.model` の `Unmeasured`
- モデル: `spec/models/rmodel-calibration.model` の `Improved`
- モデル: `spec/models/rmodel-calibration.model` の `NoGain`
- モデル: `spec/models/rmodel-calibration.model` の `Regressed`
- モデル: `spec/models/rmodel-calibration.model` の `goal`

### 意味の証拠を記録（record semantic evidence）

評価中で意味が未確認の版に、結果を記録する。

- id: `record-semantics`
- モデル: `spec/models/rmodel-calibration.model` の `recordSemantics`

### ゴールの証拠を記録（record goal evidence）

評価中でゴールが未測定の版に、測定結果を記録する。

- id: `record-goal`
- モデル: `spec/models/rmodel-calibration.model` の `recordGoal`

### 採用（adopt）

評価中の版の意味とゴールの証拠を確認し、採用に進める。

- id: `adopt`
- モデル: `spec/models/rmodel-calibration.model` の `adopt`

### 改版（revise）

次の版を評価中に戻し、証拠を取り直す。

- id: `revise`
- モデル: `spec/models/rmodel-calibration.model` の `revise`

### 候補の版（candidate revision）

改版前後の候補を区別する。

- id: `candidate-revision`
- モデル: `spec/models/evidence-adoption.model` の `Revision`
- モデル: `spec/models/evidence-adoption.model` の `First`
- モデル: `spec/models/evidence-adoption.model` の `Second`
- モデル: `spec/models/evidence-adoption.model` の `current`
- モデル: `spec/models/evidence-adoption.model` の `revise`

### レビューを終えた版（reviewed revision）

要件の節と式のレビューが済み、各節の検査が期待値と一致した版。

- id: `reviewed-revision`
- モデル: `spec/models/evidence-adoption.model` の `reviewed`
- モデル: `spec/models/evidence-adoption.model` の `recordReview`

### 意味比較が一致した版（compared revision）

遅れて届いた結果にも、検査した時点の版を保持する。

- id: `compared-revision`
- モデル: `spec/models/evidence-adoption.model` の `equivalent`
- モデル: `spec/models/evidence-adoption.model` の `lateSemantic`
- モデル: `spec/models/evidence-adoption.model` の `recordSemantics`
- モデル: `spec/models/evidence-adoption.model` の `receiveLateSemantics`

### ゴールが改善した版（improved revision）

測定した時点の版でゴールが改善した。

- id: `improved-revision`
- モデル: `spec/models/evidence-adoption.model` の `improved`
- モデル: `spec/models/evidence-adoption.model` の `recordGoal`

### 採用した版（adopted revision）

現行の候補に必要な証拠がそろった状態。

- id: `adopted-revision`
- モデル: `spec/models/evidence-adoption.model` の `adopted`
- モデル: `spec/models/evidence-adoption.model` の `adopt`

### 比較結果の版（comparison revision）

受け取った結果は実行時の版を維持する。

- id: `comparison-origin`
- モデル: `spec/models/checker-comparison.model` の `Revision`
- モデル: `spec/models/checker-comparison.model` の `First`
- モデル: `spec/models/checker-comparison.model` の `Second`
- モデル: `spec/models/checker-comparison.model` の `current`
- モデル: `spec/models/checker-comparison.model` の `revise`

### 比較する検査器（required checkers）

参照評価器と rprob の二つ。

- id: `required-checkers`
- モデル: `spec/models/checker-comparison.model` の `Checker`
- モデル: `spec/models/checker-comparison.model` の `Reference`
- モデル: `spec/models/checker-comparison.model` の `Fast`

### 検査器の結果（checker result）

到着・完了を区別し、二種類の結果を抽象化して比較する。

- id: `checker-receipt`
- モデル: `spec/models/checker-comparison.model` の `received`
- モデル: `spec/models/checker-comparison.model` の `completed`
- モデル: `spec/models/checker-comparison.model` の `valueA`
- モデル: `spec/models/checker-comparison.model` の `receiveA`
- モデル: `spec/models/checker-comparison.model` の `receiveB`
- モデル: `spec/models/checker-comparison.model` の `receiveIncomplete`

### 比較結果の発行（publish comparison）

現行の対象を二つの検査器が完了まで調べ、結果が一致したときに発行する。

- id: `comparison-publication`
- モデル: `spec/models/checker-comparison.model` の `published`
- モデル: `spec/models/checker-comparison.model` の `publish`

### 改善の判定条件（improvement prerequisites）

測定が同じ計画に属し、有効で安定した標本があり、事前に決めた改善幅に達する。

- id: `measurement-prerequisite`
- モデル: `spec/models/measurement-gate.model` の `Prerequisite`
- モデル: `spec/models/measurement-gate.model` の `PinnedRun`
- モデル: `spec/models/measurement-gate.model` の `StableSamples`
- モデル: `spec/models/measurement-gate.model` の `ThresholdMet`
- モデル: `spec/models/measurement-gate.model` の `confirmed`
- モデル: `spec/models/measurement-gate.model` の `confirm`

### 測定から発行した改善（published improvement）

条件を満たしたときにだけ発行できるゴール改善の証拠。

- id: `published-measurement`
- モデル: `spec/models/measurement-gate.model` の `GoalDecision`
- モデル: `spec/models/measurement-gate.model` の `Improvement`
- モデル: `spec/models/measurement-gate.model` の `published`
- モデル: `spec/models/measurement-gate.model` の `publishImprovement`

### 値（toggle value）

例の変数に入る二通りの値。

- id: `toggle-value`
- モデル: `spec/models/rmodel-behavior.model` の `Value`
- モデル: `spec/models/rmodel-behavior.model` の `On`
- モデル: `spec/models/rmodel-behavior.model` の `Off`

### 状態変数（state variables）

入れ替える二変数と、代入せずに保持する変数。

- id: `toggle-state`
- モデル: `spec/models/rmodel-behavior.model` の `left`
- モデル: `spec/models/rmodel-behavior.model` の `right`
- モデル: `spec/models/rmodel-behavior.model` の `update`

### 入れ替え（swap）

左が Off かつ右が On のとき、二変数を同時に入れ替える。

- id: `swap`
- モデル: `spec/models/rmodel-behavior.model` の `swap`

### 個体型（entity sort）

ドメインの個体を区別する有限の型。モデル中で使う名前に、用語の説明を添えられる。

- id: `entity`
- コード: `crates/rmodel/src/syntax.rs` の `entity_definitions`

### 到達しない操作（dead behavior）

到達できる状態と引数のどの組でもガードが成立しない操作。

- id: `dead-behavior`
- コード: `crates/rmodel/src/reference.rs` の `enabled`

### 状態の oracle（snapshot oracle）

到達した設定と状態を入力に受け取り、別のファイルで定義した命題の真偽を返す。

- id: `snapshot-oracle`
- コード: `formal/rmodel/Bank.lean` の `checkBank`

### 分割モデル（linked model）

順序付きの宣言ファイルから構成し、型名と操作名を一つのモデルで解決する。

- id: `linked-model`
- コード: `crates/rcheck/src/project.rs` の `load_source`

### ガードの境界（guard boundary）

隣接する整数入力または一遷移の差で、同じ操作を受け付けるかどうかが変わる、到達可能な二例の組。

- id: `guard-boundary`
- コード: `crates/rcheck/src/conform/boundaries.rs` の `boundary_cases`

### 残高（balance）

出金例の現在の残高。

- id: `nat-balance`
- モデル: `fixtures/rcheck/nat-account.model` の `balance`

### 残高の上限（balance limit）

出金例の設定で決まる残高の上限。

- id: `nat-limit`
- モデル: `fixtures/rcheck/nat-account.model` の `limit`

### 出金（withdraw）

正の額が残高以下なら残高から引く。

- id: `nat-withdraw`
- モデル: `fixtures/rcheck/nat-account.model` の `withdraw`

### 自然数（natural number）

整数スコープのうち0以上で、Nat の設定、操作引数や状態に使える値。

- id: `natural-value`
- コード: `crates/rmodel/src/syntax.rs` の `NAT`

### 処理前の検証（command validation）

実装の処理に渡す前に、入力の型、現在のガード、予測した状態の宣言と不変条件を検査する。

- id: `command-validation`
- コード: `crates/rcheck/src/conform/validation.rs` の `dispatch`

### 有限列（bounded sequence）

長さに上限があり、順序と重複を保持する。

- id: `bounded-sequence`
- コード: `crates/rmodel/src/structured.rs` の `Sequence`

### 値付き variant（payload variant）

タグが値の型を決め、値のないタグも持てる。

- id: `payload-variant`
- コード: `crates/rmodel/src/structured.rs` の `Variant`

### 進行性の問い（progress obligation）

指定した公平性を満たす実行について、いつか応答が起きることを求める。

- id: `progress-obligation`
- コード: `crates/rmodel/src/syntax.rs` の `TemporalCheck`

### 遷移の性質（transition property）

実行可能な一操作の直前と直後を比較する条件。

- id: `transition-property`
- コード: `crates/rmodel/src/syntax.rs` の `TransitionCheck`

## 実装の照合

- `evals/experiments/rprob/goal.py` の `score` を、`evals/experiments/rprob/test_goal.py` の `test_measurement_is_bound_to_pinned_inputs` がモデル `spec/models/measurement-gate.model` と照合する
- `crates/idealis-spec/src/comparison.rs` の `compare_receipts` を、`crates/idealis-spec/tests/comparison.rs` の `real_checker_results_feed_adoption` がモデル `spec/models/checker-comparison.model` と照合する
- `crates/idealis-spec/src/adoption.rs` の `assess_adoption` を、`crates/idealis-spec/tests/adoption.rs` の `a_late_result_cannot_authorize_the_current_candidate` がモデル `spec/models/evidence-adoption.model` と照合する
- `crates/rmodel/src/syntax.rs` の `action` を、`crates/rmodel/tests/behavior.rs` の `behavior_has_the_same_guard_parallel_updates_and_frame_as_action` がモデル `spec/models/rmodel-behavior.model` と照合する
- `crates/rmodel/src/reference.rs` の `steps` を、`crates/rmodel/tests/calibration.rs` の `the_next_revision_must_collect_new_evidence` がモデル `spec/models/rmodel-calibration.model` と照合する

## rule

### RMODEL-SEMANTICS

採用した版には意味の一致が必要。差異、空、探索未完了、比較相手の欠落は、ゴールが改善しても一致として扱わない。

- `spec/models/rmodel-calibration.model` の `AdoptionNeedsEquivalence` は holds
- `spec/models/rmodel-calibration.model` の `ImprovedButDiverges` は reachable
- `spec/models/rmodel-calibration.model` の `ImprovedButEmpty` は reachable
- `spec/models/rmodel-calibration.model` の `ImprovedButIncomplete` は reachable
- `spec/models/rmodel-calibration.model` の `ImprovedButMissingComparison` は reachable

### RMODEL-GOAL

採用した版にはゴールの改善が必要。意味だけが一致した版は評価中にでき、意味の一致とゴールの改善がそろった版は採用に進める。

- `spec/models/rmodel-calibration.model` の `AdoptionNeedsImprovement` は holds
- `spec/models/rmodel-calibration.model` の `EquivalentButUnmeasured` は reachable
- `spec/models/rmodel-calibration.model` の `SupportedAdoption` は reachable

### RMODEL-REVISION

改版した候補は評価中になり、旧版で集めた意味とゴールの証拠を取り直す。

- 形式化しない理由: 遷移の前後を比較する条件は、calibration.rs の the_next_revision_must_collect_new_evidence が参照評価器で検査する。

### RMODEL-EVIDENCE-PROVENANCE

節レビュー・意味の一致・ゴール改善を、評価した候補の版に結び付けて採用を判断する。

- `spec/models/evidence-adoption.model` の `AdoptedIsCurrent` は holds
- `spec/models/evidence-adoption.model` の `OldAgreementArrives` は reachable
- `spec/models/evidence-adoption.model` の `CurrentEvidenceCanAdopt` は reachable
- `review`　採用した版には、対応レビューと期待通りのモデル検査が必要。
  - `spec/models/evidence-adoption.model` の `AdoptionNeedsCurrentReview` は holds
- `agreement`　採用した版の意味比較が一致していなければならない。旧版の結果はその版に留める。
  - `spec/models/evidence-adoption.model` の `AdoptionNeedsCurrentAgreement` は holds
- `goal`　採用した版でゴールが改善していなければならない。
  - `spec/models/evidence-adoption.model` の `AdoptionNeedsCurrentImprovement` は holds

### RMODEL-ADOPTION-LIFETIME

版が変わらない間は、旧版の検査結果が遅れて届いても採用を保持する。版を変えたときに採用を解除する。

- `spec/models/evidence-adoption.model` の `AdoptionSurvivesSameRevision` は holds
- `spec/models/evidence-adoption.model` の `RevisionClearsAdoption` は holds

### RMODEL-EXPECTED-CHECKERS

比較対象の版で、予定した二つの検査器が完了し、判定が一致して初めて意味の一致を発行できる。

- `spec/models/checker-comparison.model` の `PublishedNeedsCurrentResults` は holds
- `spec/models/checker-comparison.model` の `PublishedNeedsAgreement` は holds
- `spec/models/checker-comparison.model` の `OneCompleteResult` は reachable
- `spec/models/checker-comparison.model` の `OldResultsOnly` は reachable
- `spec/models/checker-comparison.model` の `CurrentAgreementPublished` は reachable

### RMODEL-RECEIPT-IMMUTABILITY

一度受領した検査結果は、版と検査器の組ごとに受領記録・完了状態・判定を保持する。

- `spec/models/checker-comparison.model` の `ReceivedResultStays` は holds

### RMODEL-MEASURED-IMPROVEMENT

測定前に固定した計画に属し、標本が有効で安定し、改善幅を満たすときにゴール改善を発行する。

- `spec/models/measurement-gate.model` の `ImprovementNeedsPinnedRun` は holds
- `spec/models/measurement-gate.model` の `ImprovementNeedsStableSamples` は holds
- `spec/models/measurement-gate.model` の `ImprovementNeedsThreshold` は holds
- `spec/models/measurement-gate.model` の `UnsettledMeasurement` は reachable
- `spec/models/measurement-gate.model` の `ImprovementPublished` は reachable

### RMODEL-PREREQUISITE-AT-PUBLICATION

ゴール改善を発行するとき、その操作は確認済みの条件を変更しない。

- `spec/models/measurement-gate.model` の `PublicationKeepsPrerequisites` は holds

### RMODEL-SWAP

入れ替え例では左と右を遷移前の値から同時に更新する。二つの値は異なり続け、入れ替え後の状態に到達でき、両方が On の状態には到達しない。代入しない変数 update は Off のままにする。

- `spec/models/rmodel-behavior.model` の `Distinct` は holds
- `spec/models/rmodel-behavior.model` の `Swapped` は reachable
- `spec/models/rmodel-behavior.model` の `BothOn` は unreachable
- `spec/models/rmodel-behavior.model` の `PreservesUpdate` は holds

### RMODEL-ENTITY

個体型の宣言を型と用語の正本とする。同じ名前を二重に宣言した場合や存在しない型を参照した場合は拒否する。用語の説明は IR、Rust と TypeScript の型、用語集へ引き継ぐ。

- 形式化しない理由: 名前解決と生成物は rmodel の entity.rs、IR の往復は rcheck の ir.rs、生成した型を使う入出金の照合は rcheck の domain.rs と typescript.rs で検査する。

### RMODEL-DEAD-CODE

ガードの成立例があれば到達可能とする。成立例なしをデッドと呼べるのは、初期状態が存在し、探索が完了したとき。探索の予算切れや範囲逸脱、初期状態なしは区別する。

- 形式化しない理由: 操作の到達結果と完了状態の区別は rcheck の CLI テスト dead_code.rs で実行する。

### RMODEL-LEAN-ORACLE

完全探索した非空の範囲に限り、各設定・状態に Lean の命題を照合できる。偽ならその観測を返し、探索が未完了または Lean の出力が不正なら区別して扱う。

- 形式化しない理由: 別プロセスとの照合と失敗の区別を rcheck の lean_oracle.rs で検査する。

### RMODEL-LINKED

分割した宣言を一つのモデルとして解決し、業務上の拒否と分散プロトコルの安全性を同じ型・操作・検査で記述できる。

- 形式化しない理由: 出金と Raft の選挙の例は、マニフェストを入力にする linked_projects.rs の CLI テストで検査する。

### RMODEL-BOUNDARY

ガードの境界は宣言した有限の範囲を完全探索した後で選び、先行操作、受付結果、直後の状態を生成テストに渡す。

- 形式化しない理由: 境界の選出と探索未完了の扱いは linked_projects.rs、仕様と独立実装の差は withdrawal.pbt.test.ts で検査する。

### RMODEL-NAT

出金例は Nat の金額を使い、残高を上限以下に保つ。残高をちょうど0にすることができる。

- `fixtures/rcheck/nat-account.model` の `WithinLimit` は holds
- `fixtures/rcheck/nat-account.model` の `EmptyBalance` は reachable

### RMODEL-VALIDATE

入力・設定・状態がモデルに適合し、ガードが成立し、遷移先が範囲と不変条件を満たすときに実装の処理を呼ぶ。実装の応答がモデルと異なったセッションでは後続処理を呼ばない。

- 形式化しない理由: validation.rs のテストで実装の呼び出しを計測する。

### RMODEL-STRUCTURED

生成した値と観測は列の順序・重複と variant のタグ・値の型を保つ。

- 形式化しない理由: structured_temporal.rs で順序を入れ替えた観測を検出し、列の上限と値付きタグの型を検査する。

### RMODEL-PROGRESS

同じ配信モデルでも、問いが指定する公平性によって進行性の判定が変わる。空や探索の打ち切りを成立としない。

- 形式化しない理由: rcheck の structured_temporal.rs と TLC の 0116 評価で公平性ありとなしを照合する。

### SHIPMENT-TRACKING

注文が発送済みなら、実行可能な操作の直後も発送済みであり、追跡方法は操作直前と同じでなければならない。

- `fixtures/rcheck/structured/shipped-transition.model` の `ShippedStays` は holds
- `fixtures/rcheck/structured/shipped-transition.model` の `WasShipped` は reachable
- `status`　発送済みの注文は、実行可能な操作の直後も発送済みのままとする。
  - `fixtures/rcheck/structured/shipped-transition.model` の `StaysShipped` は holds
- `tracking`　操作の前後とも発送済みなら、追跡方法は等しい。
  - `fixtures/rcheck/structured/shipped-transition.model` の `KeepsTracking` は holds
- `confirm-courier`　Courier で発送し、追跡方法を維持したまま確認済みになる経路がある。
  - `fixtures/rcheck/structured/shipped-transition.model` の `ConfirmCourier` は reachable
- `confirm-post`　Post で発送し、追跡方法を維持したまま確認済みになる経路がある。
  - `fixtures/rcheck/structured/shipped-transition.model` の `ConfirmPost` は reachable

### RMODEL-TRANSITION

遷移の性質を破る操作は実装を呼ぶ前に検出する。

- 形式化しない理由: transition.rs で正常版と追跡方法を書き換える故障を参照評価器と rtlc で比べる。TLC は評価 0117 で照合する。
