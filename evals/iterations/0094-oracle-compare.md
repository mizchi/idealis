# 0094: roracleの`compare`を語彙の上で記述し、実装を照合する

[plan](../plans/0094-oracle-compare.md)のとおり、roracleの`compare`をrmodelで記述し（[`spec/models/roracle-compare.model`](../../spec/models/roracle-compare.model)）、実装を照合した。

- **モデル**: 観測の種類（値・許容値の集合・未確定・無効・エラー）はenumで、未確定・無効・エラーは0093と同じ用語（search.unknown・compare.invalid・compare.error）へ結びつけた。新しい用語は観測・値・許容値の集合・基準の観測・実装の観測の5語。rule 3つ・claim 6つは、ralloyのSATとrtlcの両方で期待と一致した。
- **照合**: 値{0, 1}の上の観測9種の組81通りすべてで、`compare`の結果がモデルの事実を満たした（`compare_conforms_to_the_compare_model`）。
- **故障**: 空の許容値の集合を不一致にする故障（`Allowed([]) vs Value(0) -> Mismatch`）と、実装の観測が値でないときに等値で一致にする故障（`Allowed([]) vs Allowed([]) -> Matched`）を、照合が事実の違反として検出した。

## 見つかったralloyのバグ

モデルにenumが2つあり、Alloyでは`one sig`が10個、全体で14 sigになった。ralloyのparserはsigを16個まで受け付けるが、型検査は組み込みのInt・Stringを足した数をu16のbit maskで表していて、15個以上でshiftが桁あふれした（debugではpanic、releaseでは黙ってmaskが0になり、型の区別が壊れる）。16 sigのモデルのテストを先に書いて失敗を確かめ、maskをu32へ広げた。

## jev-lint（jev-1.14.0、UNREVIEWED）

追加した3つのruleとモデルの比較はすべてsupports。ralloyのREADMEは引き続き矛盾（0.65）とされ、根拠を特定できない候補のまま。[記録](../runs/0094-oracle-compare/drift.json)。

## 後からの変更

照合の2つのテストは、roracleの中からrspecのテスト（`crates/rspec/tests/grounding.rs`）へ移した。roracleがrmodelとralloyをdev-dependencyに持つと、配布するcrateだけで組み立てる`just package`のworkspaceにrmodelがなく、CIで失敗したため。roracleは、照合用の入口を`#[doc(hidden)] pub mod grounding`として公開する（安定したAPIではない）。
