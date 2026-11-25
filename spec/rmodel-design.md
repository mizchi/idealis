# rmodel の文法設計仕様

<!-- Generated from spec/rmodel-design.pkl by rspec (plan 0092). Do not edit. -->

## 文法変更と遷移

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

## 実装の照合

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
