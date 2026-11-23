# idealis の語彙

<!-- Generated from spec/idealis.pkl by rspec (plan 0092). Do not edit. -->

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
- コード: `crates/roracle/src/report.rs` の `Matched`
- コード: `crates/ralloy/src/lib.rs` の `Matched`

### 不一致（mismatch）

観測は正しく取れたうえで、基準の答えと異なったこと。

- id: `compare.mismatch`
- コード: `crates/roracle/src/report.rs` の `Mismatch`
- コード: `crates/ralloy/src/lib.rs` の `Mismatch`

### 無効（invalid）

入力や観測が契約を満たさず、比較そのものが成立しなかったこと。不一致とは区別する。

- id: `compare.invalid`
- コード: `crates/roracle/src/report.rs` の `Invalid`
- コード: `crates/ralloy/src/lib.rs` の `Invalid`

### oracle（oracle）

比較の基準にする、別の道具（Alloy・TLC・Z3 など）の答え。記録して再生し、記録時の版と入力の digest を残す。

- id: `compare.oracle`
- コード: `crates/roracle/src/report.rs` の `Status`

## モデル

### 設定（configuration）

状態とともに変わらない関係（rmodel の rel）。solver はすべての候補を考え、明示状態の探索は初期状態で列挙する。

- id: `model.configuration`
- コード: `crates/rmodel/src/syntax.rs` の `Relation`

### 状態（state）

action によって変わる関係（rmodel の var、Alloy の var field）の、ある時点の値。

- id: `model.state`
- コード: `crates/rmodel/src/syntax.rs` の `var`

### action（action）

状態を変える遷移。条件と代入を持ち、代入しない状態は変わらない（暗黙の frame）。

- id: `model.action`
- コード: `crates/rmodel/src/syntax.rs` の `Action`

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
- コード: `crates/rmodel/src/syntax.rs` の `Scope`
