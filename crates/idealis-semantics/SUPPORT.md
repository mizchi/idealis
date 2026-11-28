# idealis-semantics の対応範囲

- [x] 有限の設定・初期状態・有効な操作を探索し、状態と遷移の性質を判定する。空、予算切れ、スコープ逸脱、宣言違反を区別する。[参照評価器](src/reference.rs) · [単独利用の試験](tests/replay.rs)
- [x] 反例の操作列を復元し、コマンドと状態を再生する。ソースの分岐情報を使った到達性検査と観測履歴の比較も扱う。[コマンドの再生](src/reference/input.rs) · [観測履歴](src/reference/history.rs)

- [ ] SAT や TLA+ への変換、検査器の選択。[検査と照合](../idealis-check/SUPPORT.md)
