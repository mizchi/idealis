# workspaceの差分検査とチューニング

既存evalの差分assertionを、roracleのRust照合器へ通す。Node adapterはWasmを1 instanceだけ遅延ロードする。[移行一覧](../../../fixtures/workspace-oracle/migrations.json)には23ファイル・99箇所を記録した。parserの正規化、Z3のSAT witness検証、TLCの分類と強制trace検証は各adapterに残している。TS/Rust/Goの共通SUTと非同期履歴も同じassertionを使う。metadataの検査や単独の期待値テスト、過去のraw artifactは一括変換していない。

## 差分テストに使う

```ts
import { assertOracle, compareOracle } from "./node.js";

assertOracle(actual, reference, "native/Wasm reply");
const checked = compareOracle(actual, reference);
```

assertOracleは不一致でNodeのAssertionErrorを投げ、reportも保持する。入力が表す操作とモデルの観測を渡したい場合は、wasm.tsのbatch requestへcaseのinputを指定する。共通assertionのcase inputは診断用labelで、操作列を自動収集する機能ではない。

snapshotはplain object・dense arrayとscalarを型付きで符号化する。undefined、bigint、NaN、Infinity、負のzeroを区別し、objectのkey順を比較しない。循環、class instance、Map/Set、symbol、accessor、非列挙property、疎なarrayは明示的に拒否する。深さは48まで。JSの任意objectグラフを輸送する契約ではない。

request/reportのTS宣言はroracleのRust型から生成する。`just oracle-types-check`で鮮度を確認する。Node限定のnapi-rs比較は[実験binding](../roracle-native/README.md)を参照。`IDEALIS_ORACLE_BACKEND`で明示選択できるが、既定はWasmのまま。共通の全report decoderを使い、nativeの欠落時にfallbackしない。

ここでの応答一致はprotocol回帰の意味。意図したエラーやunknownを含む応答snapshotの一致を、モデルの安全性やSATの成功にしない。新しい実装の意味論を検証するには、独立したオラクルとdomainの判定を使う。

## 変更前後を保存して比較する

Rustとwasm32-unknown-unknown、Node 24+、pnpm、justが必要。

```sh
just workspace-oracle-test
just workspace-snapshot .artifacts/tuning-before
# コードを修正し、対象crateをrelease buildして回帰を確認する
just workspace-snapshot .artifacts/tuning-after
just workspace-bench .artifacts/tuning-before .artifacts/tuning-after evals/runs/my-tuning
just workspace-replay evals/runs/my-tuning evals/runs/my-tuning-replay
```

snapshotは6実行境界のbinaryとsourceを別directoryへコピーし、hash・revision・環境・caseを保存する。coreの境界はrtla/rtla-compiler/rtlcを含む。caseのrequest・ID・受理条件が異なるsnapshotは比較しない。baselineとcandidateのbinaryが同じsnapshot同士も、測定器の正常対照として実行できる。

13 casesをcold/reusedで分け、12 groupで実行する。referenceはbaseline専用の別process、速度baselineもbaselineを明示する。coldは毎回起動し、reusedは3つの独立した常駐JSONL processを使う。起動やIPC、JSONの費用を含む。coldは10 samples・2 warmup呼出・1 iteration、reusedは20 samples・75 warmup呼出・25 iterationsで、全呼出結果を保存したreferenceへ照合する。受理条件はunknown/boundedや意図しないエラーを拒否する。

異常終了、応答欠落、不正JSON、途中の結果変化を成功にしない。最後のJSONを返してから異常終了した場合や余分なstdoutが残る場合も、process終了時に検査して全速度比を無効化する。自動timeoutは設けていない。任意の外部programの停止は呼び出し側が管理する。statefulなhandle/sessionを含める場合は、その初期化・解放を独立したcaseへ設計する。

runのinput、preflight、raw ns、順序、統計、source/binary hashを保存する。replayは各targetを保存referenceへ照合し、reference processは呼ばない。上書きを防ぐため、新しいdirectoryを指定する。sourceの異なる変更前後は許可し、snapshot内のsource/binaryの改変は拒否する。

## bridge費用とkernelの最適化を分ける

```sh
just workspace-oracle-cost evals/runs/my-oracle-cost
just workspace-oracle-cost-replay evals/runs/my-oracle-cost evals/runs/my-oracle-cost-replay
```

従来のNode strict equalityと新しいsnapshot/Wasm照合を、同じJSONL runnerで比較する。実parser/coreの応答を使い、coldは10 samples・2 warmup呼出・1 iteration、reusedは20 samples・75 warmup呼出・25 iterationsで測る。Nodeの常駐process、IPCとJSONを含むため、Wasmの純粋な関数時間とは呼ばない。

検査kernelの改善は既存のCriterion/profileを使う。このrunnerは、その変更前後の応答が保たれるかと、呼び出し境界全体の費用を確認する。測定中はbuild、test、別のsolver/profileを並走させない。

既存GoのEOF入力型CLIは、そのまま常駐JSONLとして使えない。実装比較はcross/asyncの既存runnerで行い、汎用process runnerへ接続する場合はcoldまたはJSONL adapterを使う。

## 通信と照合の変更を分けて測る

0054では親processの小さなpipe writeがCPU profileの約79%を占めた。BufWriterでJSONの書き込みをまとめ、request serializationは時計内に残す。Wasm照合器はJSON観測のコピーと中間reportを減らし、全応答とsnapshotの表現を維持する。

```sh
# 対象をbuildして回帰確認後、変更前を保存する
just oracle-tuning-freeze .artifacts/oracle-before
# transportだけを変更してrelease buildする
just oracle-tuning-freeze .artifacts/oracle-transport
# bridgeも変更してnative/Wasm/TSをbuildする
just oracle-tuning-freeze .artifacts/oracle-after
just oracle-tuning-compat .artifacts/oracle-before .artifacts/oracle-after evals/runs/oracle-compat
just oracle-tuning-measure .artifacts/oracle-before .artifacts/oracle-transport .artifacts/oracle-after evals/runs/oracle-roundtrip
```

測定順はbefore/transport/bridge/bridge/transport/before。bridgeはtransportと同じ親runnerを使い、各stageのlegacy Node比較も対照として保存する。固定した実rtla/core応答を使い、20 samples・75 warmup呼出・25 iterations、予定呼出上限10000で測る。snapshotにはJS adapterとWasmもコピーし、frozen workerの作業directoryを固定する。

Criterionは同じ`benches/wire.rs`でbuildした`wire-bench`とhashを保存してから、`node dist/evals/experiments/oracle-tuning/kernel.js <before> <after> <new-run>`でbefore/after/after/beforeを実行する。各snapshotに実行ファイル`wire-bench`と`criterion.json`（binarySha256、benchSha256）を置く。出力の各directoryにCriterionのraw sampleと推定値を保存する。通常のbenchは`just oracle-criterion`を使う。

[0054の結果](../../iterations/0054-oracle-bench-tuning.md)には通信・bridge・kernelを別々に記録する。固定応答の照合費用から、TLCやZ3との探索速度比は求めない。過去の7/1/3の測定とも混ぜない。

## 残る処理費用を調べる

```sh
just oracle-tuning-phases .artifacts/my-before evals/runs/my-profile profile
just oracle-tuning-phases .artifacts/my-before evals/runs/my-phases
# Rustの変更だけをfreezeし、次にJS側の変更もfreezeする
just oracle-tuning-compare .artifacts/my-before .artifacts/my-wire .artifacts/my-combined evals/runs/my-compare
node dist/evals/experiments/oracle-tuning/snapshot-compat.js .artifacts/my-before .artifacts/my-combined evals/runs/my-snapshot-compat
```

phasesはsnapshot・JSON文字列化・UTF-8 encode・Wasm dispatch・decodeとparse・全compareOracleを分ける。入力と全出力を照合し、正順/逆順のraw sampleを保存する。profileは別runで5000回の全照合を記録する。区間測定には時計とloopの費用も入るので、IPC込みの往復やCriterionと混ぜて速度比を作らない。

compareはbefore/wire/combined/combined/wire/beforeを、全stageでbeforeの同じ親runnerで実行する。各snapshotのsource/binary hashを検査する。[0055の結果](../../iterations/0055-oracle-hotspots.md)では公開Observationのserdeとwire parserを照合し、snapshotの全reportと拒否入力も旧版と比較した。
