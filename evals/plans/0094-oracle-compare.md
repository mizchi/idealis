# 0094: roracleの`compare`を語彙の上で記述し、実装を照合する

[0093](0093-implementation-behavior.md)と同じ形で、roracleの`compare`（基準の観測と実装の観測から、比較のstatusを出す規則）をrmodelで記述し、実装を照合する。

## 変更

- `spec/models/roracle-compare.model`: 基準と実装の観測の種類（値・許容値の集合・未確定・無効・エラー）と値、比較の結果。規則:
  - どちらかが未確定・無効・エラーなら、それらの最も重いもの（エラー > 未確定 > 無効）。
  - そうでなく、実装の観測が値でなければ無効。
  - 基準が値なら、等しければ一致、違えば不一致。基準が空でない許容値の集合なら、含まれれば一致、なければ不一致。空の集合なら無効。
- `spec/idealis.pkl`: 宣言を語彙へ結びつけ、ruleとclaim、groundingを加える。
- 照合: 値{0, 1}の上の観測9種（値2、許容値の集合4、未確定・無効・エラー）の組81通りで、`compare`の結果がモデルの事実を満たすこと。

## 正しさ（結果を見る前に固定）

- check（SATとrtlcで一致すること）:
  - `MatchedNeedsActualValue`（一致なら実装の観測は値。invariant）: holds
  - `FailureNeverMatched`（どちらかが未確定・無効・エラーなら一致でない。invariant）: holds
  - `ErrorDominates`（どちらかがエラーなら結果はエラー。invariant）: holds
  - `ActualAllowedIsInvalid`（基準が値で実装が許容値の集合なら無効。invariant）: holds
  - `EmptyAllowedIsInvalid`（空の許容値の集合と値を比べて無効になる。reachable）: reachable
  - `AllowedMismatch`（空でない許容値の集合に含まれない値で不一致。reachable）: reachable
- 照合: 81通りすべてで事実を満たす。
- 人工故障: 実装で空の許容値の集合を不一致にする、実装の観測が許容値の集合のときを一致扱いにする（等値で比べる）、の2件を照合が検出すること。

## 範囲外

- 型ごとの等値（`PartialEq`）の意味。値は2つのatomで表す。
