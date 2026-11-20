# 明示した並行スケジュールの評価

`idealis-connect`のScheduleをnative/Wasmで構築し、TSのasync generatorとチャネルでgateした実Rust threadを同じactor順で進める。対象は2/3 actorによる既知のlost updateと正常なincrement。期待値は完了したwrite数で、SUTの観測を転用しない。

```sh
just controlled-test
just controlled-browser
just controlled-replay evals/runs/0038-packaging-regression-development
just controlled-replay evals/runs/0038-packaging-regression-confirmation
```

Rust、`wasm32-unknown-unknown`、Node.js 24+、pnpmとChromiumが必要。`controlled-test`はpure coreのRust tests、native gate、Loom 0.7.2、node:testとVitestを実行する。`controlled-browser`は実ブラウザで生成・縮小・再生し、反例をnative gateで再生する。

[事前計画](../../plans/0038-controlled-schedules.md)の開発seedは0x38a1から16件、確認は0x38c3から32件。各seedで2/3 actor、正常版と故障版を検査し、normalは64有効caseを要求する。回帰・ブラウザ用の0x3801は独立した確認実験へ数えない。

`just controlled-eval development <新規run>`と`just controlled-eval confirmation <新規run> <開発run>`で記録する。既存ディレクトリは拒否する。ソース・依存lock・binary hash、全trial、core/SUTのnative輸送記録、Loomの最終値集合を保存する。再生は同じソース・binaryで全記録を比較する。結果を見たseedでの反復は回帰確認と扱う。

元の開発・確認runはcommit `9317472`のsourceで再生する。最終版では単独Rust consumerのexampleだけを修正し、同じseedの回帰runを上のコマンドで再生できる。[修正前に記録した条件](../../plans/0038-packaging-regression.md)を参照。回帰を新しい独立した確認実験には数えない。

完了までの全操作列は、独立TS再帰と生成器の列挙を照合する。2 actorは6通り、3 actorは90通り。PBTは全列挙とは分けて集計する。Loomはnativeの参照例だけで使い、[公式に記載された制約](https://github.com/tokio-rs/loom/tree/v0.7.2#unsupported-features)を含む。今回の一致は全C11/SeqCstの互換や任意のRust threadの証明ではない。

[0038の採否と結果](../../iterations/0038-controlled-schedules.md)と[公開APIの使い方](../../../docs/testing.md#actorの操作順を制御する)を参照。新しいcore依存はなく、LoomやOS gateは公開Wasmへ入れない。
