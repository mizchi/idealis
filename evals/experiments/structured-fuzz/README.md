# testing protocolの構造付きfuzzing

非公開eval crateで、`idealis-connect::Engine`と保存caseを検査する。生JSONLを変異させる`raw`と、Arbitraryから有効なcommandとfeedbackを作る`structured`を使う。対象はprotocolの状態と応答で、TLA+/Alloy/SMTの構文や実装SUTの全動作ではない。

```sh
rustup toolchain install nightly-2026-08-27 --profile minimal
cargo install cargo-fuzz --version 0.13.2 --locked
just fuzz-test
just fuzz-browser
just fuzz-replay evals/runs/0039-fuzz-development
just fuzz-replay evals/runs/0039-fuzz-confirmation
```

Node.js 24+、pnpm、Rustの`wasm32-unknown-unknown`とChromiumも必要。既存のdefault toolchainは変更しない。保存済みrunのreplayには同じsource/binaryを保持した環境が必要。別の環境では新規runを作り、そのrunを再生する。同じseedの再実行は回帰として扱う。評価用binaryのpathはMac arm64とLinux x86_64を扱う。今回のcampaignはMac arm64で実行し、CIのASan smokeはLinux向けに追加した。

## 何を検査するか

pending IDとcaseを使ってresumeし、生成・縮小・確認・再生まで進める。不正requestの後は、そのrequestを除く履歴から別Engineを復元し、次の応答を照合する。同じ実装の別instanceへ異なる履歴を渡すmetamorphicな検査であり、独立したモデル検査アルゴリズムとの比較ではない。

passedには有効caseがあり、failedには同じsignatureとcaseの最終mismatchがあることを検査する。ここでのmismatch feedbackはfuzzerが合成したもの。failed応答の件数をSUTの不具合発見数へ数えない。pending sessionを意図的に消す対照では、照合器が不一致を拒否する。

各入力は最大8192 bytes、64 command、参照側の再構築を含めて256 Engine呼び出し。自動feedbackも上限を設け、残った入力やsessionはboundedと記録する。部分実行を全入力の検査成功とは扱わない。構造付き入力はArbitraryによる生成で、custom mutatorは今回使っていない。[Rust Fuzz Bookの構造付きfuzzing](https://rust-fuzz.github.io/book/cargo-fuzz/structure-aware-fuzzing.html)を参照。

## 実行と保存

`just fuzz-eval development <新規run>`と`just fuzz-eval confirmation <新規run> <開発run>`で実行する。各targetは10,000 runs、入力長8192、RSS上限1024 MiB。libFuzzerの既定の時間制限は0で無効にし、run数で停止する。非ゼロ終了、signal、RSS超過を成功にしない。

初期corpusは0036/0038の固定した保存記録から作る。開発・確認はそれぞれ同じ初期corpusから開始し、開発で増えたcorpusを確認へ持ち越さない。plan・source version・lock・binary hash・実行引数・生ログと終了codeを保存する。保存したcorpusは初期入力とcoverageで残った入力であり、実行した全10,000入力の保存ではない。

runの`corpus.json.gz`は全保存入力をbase64とSHA-256で持つ。`artifacts.json.gz`はクラッシュ等のartifact、`records.json.gz`は通常nativeとWasmの全要求・応答、`asan-replay.json.gz`は固定ファイルのASan再生ログ。rawの作業ディレクトリも実行中は残すが、Gitへは圧縮archiveを記録する。既存runは上書きしない。

`fuzz-replay`は同じsource/binaryを要求し、archiveを別ディレクトリへ復元する。ASanで各固定入力を実行し、通常native/Wasmでも保存応答を再生する。成功応答は完全一致を要求する。不正JSONは両方がnamed errorであることを要求し、非対応値域に関するplatform依存のparser詳細は一致条件へ含めない。

coverageの進み方やcorpusの選択は環境に依存する。seedを固定してcampaignを再実行しても同じcorpusになるとは主張しない。保存入力の再現と、新しい探索を分ける。実Chromiumでは固定回帰transcriptを再生し、ブラウザ上で全campaignを実行したとは扱わない。

## CIと採用範囲

通常CIはRust tests、node:test/Vitest、実Chromiumを実行する。`Fuzz smoke` workflowは固定nightlyとcargo-fuzzを使い、回帰seed 0x3901で各1,000 runsを実行する。CIの反復は新しい独立確認実験に数えない。

[0039の結果と採否](../../iterations/0039-structured-fuzz.md)を参照。protocolと保存caseの回帰targetとして採用する。有限runでpanicが見つからなかった結果を、Engineや形式仕様全体の正しさへ一般化しない。fuzzerの依存とnightlyは公開coreやWasmに入れない。
