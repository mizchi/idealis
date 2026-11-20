# testing coreの評価

[事前計画](../../plans/0036-testing-core.md)に従い、同じcorpusをRust native/Wasmと既存TS実装へ渡す。公開APIは[利用ガイド](../../../docs/testing.md)を参照。

```sh
just testing-test
just testing-browser
just testing-eval development evals/runs/my-testing-development
just testing-eval confirmation evals/runs/my-testing-confirmation evals/runs/my-testing-development
just testing-replay evals/runs/my-testing-confirmation
```

開発seed0x36a1から16件、確認seed0x36c3から32件を使う。各seedで正常版と既知の永続化故障版のdurable counterを実行する。正常版は16有効caseを要求する。Rust/Wasmの結果と全trial、TS生成器の操作列、既存TS縮小器の出力・check数、既存TS差分比較を照合する。

返却caseを故障版と正常版で再実行し、保存caseの再生も検査する。既知の人工故障であり、未知バグ発見には数えない。fast-checkの任意Strategyやseed互換、性能優位を主張する評価ではない。

manifestへ全ソースとnative/Wasmのhash、seed、環境を保存する。records.json.gzには全trialと最終trace、exchanges.json.gzにはnativeの全request/replyを保存する。replayは証拠hashと同じソース・binaryを確認し、全記録を再現する。既存runへ上書きせず、新しいIDを使う。
