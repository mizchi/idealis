# idealis の語彙

<!-- Generated from spec/idealis.pkl by idealis-spec (plan 0092). Do not edit. -->

## 探索の範囲

### 完全（complete）

宣言した scope と上限の中で、到達できる状態や候補をすべて調べ終えたこと。その範囲の外については何も示さない。

- id: `search.complete`
- この意味に使わない語: 証明済み、proved
- コード: `crates/rtlc/src/safety.rs` の `complete`

### 有界（bounded）

深さ・steps・状態数・work などの上限で探索を打ち切り、調べた部分に違反がなかったこと。どの上限かは理由（rtlc の reason）で示す。上限の外は調べていないので、完全な検査として扱わない。

- id: `search.bounded`
- この意味に使わない語: 完全、網羅、exhaustive
- コード: `crates/rtlc/src/safety.rs` の `bounded`
- コード: `crates/ralloy/src/lib.rs` の `steps`

### 空（empty）

初期状態や候補が 1 つもなく、検査する対象がなかったこと。合格ではない。

- id: `search.empty`
- コード: `crates/rtlc/src/safety.rs` の `empty`

### 未確定（unknown）

予算が尽きて、実例・反例の有無という判定そのものを出せなかったこと。合格にも不合格にも読み替えない。work の予算切れは、ralloy では unknown、rtlc では理由 work の bounded になる（0092 で記録した語の違い）。

- id: `search.unknown`
- モデル: `spec/models/roracle-report.model` の `Unknown`
- モデル: `spec/models/roracle-compare.model` の `Unknown`
- モデル: `spec/models/roracle-compare.model` の `UnknownObservation`
- コード: `crates/ralloy/src/lib.rs` の `Unknown`
- コード: `crates/roracle/src/report.rs` の `Unknown`

## 判定

### 実例（instance）

run の条件と事実を満たす構造（時相モデルでは trace）が scope の中に見つかったこと。

- id: `verdict.instance`
- コード: `crates/ralloy/src/lib.rs` の `Instance`

### 反例（counterexample）

事実を満たしながら、検査する性質を破る構造または trace が scope の中に見つかったこと。

- id: `verdict.counterexample`
- コード: `crates/ralloy/src/lib.rs` の `Counterexample`

### 反例なし（no counterexample）

scope の中に反例がなかったこと。scope の外で性質が成り立つことの証明ではない。

- id: `verdict.no-counterexample`
- この意味に使わない語: 正しい、検証済み、verified
- コード: `crates/ralloy/src/lib.rs` の `NoCounterexample`

### 成り立つ（holds）

invariant が、検査した範囲で到達できるすべての状態で成り立つこと。範囲が有界か完全かを併せて示す。

- id: `verdict.holds`

### 破れる（violated）

invariant が偽になる状態に到達できること。到達する trace が witness になる。

- id: `verdict.violated`

### 到達できる（reachable）

reachable の条件を満たす状態に、初期状態から到達できること。

- id: `verdict.reachable`

### 到達できない（unreachable）

検査した範囲では、reachable の条件を満たす状態に到達できないこと。範囲が有界か完全かを併せて示す。

- id: `verdict.unreachable`

## 比較

### 一致（matched）

実装や engine の観測が、基準（oracle やモデル）の答えと同じだったこと。

- id: `compare.matched`
- モデル: `spec/models/roracle-report.model` の `Matched`
- モデル: `spec/models/roracle-compare.model` の `Matched`
- コード: `crates/roracle/src/report.rs` の `Matched`
- コード: `crates/ralloy/src/lib.rs` の `Matched`

### 不一致（mismatch）

観測は正しく取れたうえで、基準の答えと異なったこと。

- id: `compare.mismatch`
- モデル: `spec/models/roracle-report.model` の `Mismatch`
- モデル: `spec/models/roracle-compare.model` の `Mismatch`
- コード: `crates/roracle/src/report.rs` の `Mismatch`
- コード: `crates/ralloy/src/lib.rs` の `Mismatch`

### 無効（invalid）

入力や観測が契約を満たさず、比較そのものが成立しなかったこと。不一致とは区別する。

- id: `compare.invalid`
- モデル: `spec/models/roracle-report.model` の `Invalid`
- モデル: `spec/models/roracle-compare.model` の `Invalid`
- モデル: `spec/models/roracle-compare.model` の `InvalidObservation`
- コード: `crates/roracle/src/report.rs` の `Invalid`
- コード: `crates/ralloy/src/lib.rs` の `Invalid`

### エラー（error）

実装・oracle・基盤が失敗して、比較する観測そのものが得られなかったこと。不一致とも未確定とも区別する。

- id: `compare.error`
- モデル: `spec/models/roracle-report.model` の `Error`
- モデル: `spec/models/roracle-compare.model` の `Error`
- モデル: `spec/models/roracle-compare.model` の `ErrorObservation`
- コード: `crates/roracle/src/report.rs` の `Error`

### 比較の結果（comparison status）

1 つの比較が一致・不一致・無効・未確定・エラーのどれだったか。

- id: `compare.status`
- モデル: `spec/models/roracle-report.model` の `Status`
- モデル: `spec/models/roracle-report.model` の `status`
- モデル: `spec/models/roracle-compare.model` の `Outcome`
- モデル: `spec/models/roracle-compare.model` の `outcome`
- コード: `crates/roracle/src/report.rs` の `Status`

### 観測（observation）

実装や oracle から得た 1 回分の答え。値・許容値の集合・未確定・無効・エラーのいずれか。

- id: `compare.observation`
- モデル: `spec/models/roracle-compare.model` の `Observed`
- コード: `crates/roracle/src/report.rs` の `Observation`

### 値（value）

正しく得られた観測の中身。基準の値とは等しさで比べる。

- id: `compare.value`
- モデル: `spec/models/roracle-compare.model` の `ValueOf`
- モデル: `spec/models/roracle-compare.model` の `Value`
- コード: `crates/roracle/src/report.rs` の `Value`

### 許容値の集合（allowed set）

基準が 1 つの値に決まらないときに、受け入れる値をすべて並べたもの。空の集合とは比較が成立しない。

- id: `compare.allowed`
- モデル: `spec/models/roracle-compare.model` の `AllowedSet`
- コード: `crates/roracle/src/report.rs` の `Allowed`

### 基準の観測（reference observation）

比較の基準になる、oracle やモデルの観測。

- id: `compare.reference`
- モデル: `spec/models/roracle-compare.model` の `referenceKind`
- モデル: `spec/models/roracle-compare.model` の `referenceValue`
- モデル: `spec/models/roracle-compare.model` の `referenceAllowed`
- コード: `crates/roracle/src/report.rs` の `reference`

### 実装の観測（actual observation）

検査する実装から得た観測。値でなければ一致にならない。

- id: `compare.actual`
- モデル: `spec/models/roracle-compare.model` の `actualKind`
- モデル: `spec/models/roracle-compare.model` の `actualValue`
- モデル: `spec/models/roracle-compare.model` の `actualAllowed`
- コード: `crates/roracle/src/report.rs` の `actual`

### 比較の枠（comparison slot）

モデルで比較の数を変えるための枠。結果を持つ枠だけが比較になる。

- id: `compare.slot`
- モデル: `spec/models/roracle-report.model` の `Slot`

### 報告の結果（report status）

比較の結果を 1 つにまとめた結果。最も重いものを示し、重い順にエラー・不一致・未確定・無効・一致。比較が 1 つもなければ無効。

- id: `compare.report`
- モデル: `spec/models/roracle-report.model` の `report`
- コード: `crates/roracle/src/report.rs` の `CheckReport`
- コード: `crates/roracle/src/report.rs` の `aggregate`

### oracle（oracle）

比較の基準にする、別の道具（Alloy・TLC・Z3 など）の答え。記録して再生し、記録時の版と入力の digest を残す。

- id: `compare.oracle`
- コード: `crates/roracle/src/report.rs` の `Status`

## モデル

### 設定（configuration）

状態とともに変わらない関係（rmodel の rel）。solver はすべての候補を考え、明示状態の探索は初期状態で列挙する。

- id: `model.configuration`
- コード: `crates/idealis-syntax/src/syntax.rs` の `Relation`

### 状態（state）

action によって変わる関係（rmodel の var、Alloy の var field）の、ある時点の値。

- id: `model.state`
- コード: `crates/idealis-syntax/src/syntax.rs` の `var`

### action（action）

状態を変える遷移。条件と代入を持ち、代入しない状態は変わらない（暗黙の frame）。

- id: `model.action`
- コード: `crates/idealis-syntax/src/syntax.rs` の `Action`

### trace（trace）

初期状態から始まる状態の列。有界な検査では、最後の状態から戻る位置を持つ lasso として表す。

- id: `model.trace`
- 同じ意味の語: lasso
- コード: `crates/ralloy/src/lib.rs` の `trace`
- コード: `crates/ralloy/src/lib.rs` の `back`

### witness（witness）

判定を裏付ける具体的な実例・反例・trace。engine とは独立した評価器で検査し直せるもの。

- id: `model.witness`
- コード: `crates/ralloy/src/lib.rs` の `check_instance`

### scope（scope）

検査する atom の数・整数の bit 幅・steps などの範囲。判定はすべてこの範囲についてのもの。

- id: `model.scope`
- コード: `crates/ralloy/src/lib.rs` の `Scope`
- コード: `crates/idealis-syntax/src/syntax.rs` の `Scope`

## TLA+ の処理系

### TLA（TLA）

Temporal Logic of Actions。状態と状態間の変化を扱い、実行列全体について性質を記述する時相論理。

- id: `tla`
- 種類: `logic`

### TLA+（TLA+）

TLA、集合論、一階述語論理を土台とする形式仕様言語。状態、次状態への遷移、実行列が満たす性質をモジュールに記述する。

- id: `tla-plus`
- 種類: `language`

### SANY（SANY）

TLA+ の公式ツール群の構文解析・意味解析器。Idealis では対応する構文や演算子の優先順位を照合する基準として使う。

- id: `sany`
- 種類: `parser`
- 入力: `tla-plus`

### TLC（TLC）

実行可能な TLA+ 仕様を検査・シミュレーションする公式ツール。Idealis では、対応する入力でモデル検査の判定や反例を照合する基準として使う。

- id: `tlc`
- 種類: `checker`
- 入力: `tla-plus`、`tlc-config`

### rtla（rtla）

TLA+ ソースと TLC 設定を型付きの構文木へ変換する Idealis の Rust パーサー。

- id: `rtla`
- 種類: `parser`
- 入力: `tla-plus`、`tlc-config`
- 出力: `rtla-ast`
- 比較先: `sany`
- コード: `crates/rtla/src/parser.rs` の `parse_module`

### rtla-compiler（rtla-compiler）

rtla の TLA+ モジュールと TLC 設定から名前を解決し、対応する仕様を rtlc の SafetyProgram に変換する Rust コンパイラー。

- id: `rtla-compiler`
- 種類: `compiler`
- 入力: `rtla-ast`
- 出力: `safety-program`
- コード: `crates/rtla-compiler/src/source.rs` の `compile_source_bundle`

### rtlc（rtlc）

SafetyProgram を入力に状態を列挙し、対応する有限モデルの不変条件・デッドロック・時相性質を調べる Idealis の Rust モデル検査ライブラリ。

- id: `rtlc`
- 種類: `checker`
- 入力: `safety-program`
- 比較先: `tlc`
- コード: `crates/rtlc/src/ir.rs` の `SafetyProgram`

### TLC 設定（TLC configuration）

初期条件、遷移、定数、検査する性質などを選ぶ TLC の設定ファイル。

- id: `tlc-config`
- 種類: `artifact`

### rtla AST（rtla AST）

rtla が解析した TLA+ のモジュールと TLC 設定の型付き構文木。

- id: `rtla-ast`
- 種類: `artifact`

### SafetyProgram（SafetyProgram）

状態の生成、遷移、検査する性質を rtlc に渡す型付きの中間表現。

- id: `safety-program`
- 種類: `artifact`

## 実装の照合

- `crates/roracle/src/report.rs` の `compare` を、`crates/idealis-spec/tests/grounding.rs` の `compare_conforms_to_the_compare_model` がモデル `spec/models/roracle-compare.model` と照合する
- `crates/roracle/src/report.rs` の `aggregate` を、`crates/idealis-spec/tests/grounding.rs` の `aggregate_conforms_to_the_report_model` がモデル `spec/models/roracle-report.model` と照合する

## rule

### ORACLE-NO-FALSE-MATCH

報告が一致になるのは、比較が 1 つ以上あり、そのすべてが一致したときだけ。未確定の比較があれば一致にならない。

- `spec/models/roracle-report.model` の `MatchedOnlyIfAllMatched` は holds
- `spec/models/roracle-report.model` の `UnknownNeverMatched` は holds

### ORACLE-HEAVIEST-WINS

報告は最も重い比較の結果を示す。不一致があれば、無効の比較があっても不一致になり、エラーと不一致がなければ未確定が無効より重い。

- `spec/models/roracle-report.model` の `InvalidHiddenByMismatch` は reachable
- `spec/models/roracle-report.model` の `UnknownOverInvalid` は holds

### ORACLE-INVALID-IS-NOT-MISMATCH

無効の比較は、不一致の比較がない限り、報告を不一致にしない。

- `spec/models/roracle-report.model` の `InvalidNotMismatch` は holds

### ORACLE-FAILURES-FIRST

基準か実装の観測が未確定・無効・エラーなら、比較は一致にならず、エラーがあれば必ずエラーになる。

- `spec/models/roracle-compare.model` の `FailureNeverMatched` は holds
- `spec/models/roracle-compare.model` の `ErrorDominates` は holds

### ORACLE-ACTUAL-IS-A-VALUE

一致するのは実装の観測が値のときだけ。どちらの観測も未確定・無効・エラーでないとき、実装が許容値の集合を返したら、比較は無効。

- `spec/models/roracle-compare.model` の `MatchedNeedsActualValue` は holds
- `spec/models/roracle-compare.model` の `ActualAllowedIsInvalid` は holds

### ORACLE-ALLOWED-SETS

どちらの観測も未確定・無効・エラーでないとき、基準が許容値の集合なら、含まれる値は一致、含まれない値は不一致で、空の集合との比較は無効。

- `spec/models/roracle-compare.model` の `EmptyAllowedIsInvalid` は reachable
- `spec/models/roracle-compare.model` の `AllowedMismatch` は reachable

### ORACLE-EMPTY-IS-INVALID

比較が 1 つもない報告は無効。

- `spec/models/roracle-report.model` の `EmptyIsInvalid` は reachable
