# 実行環境を rmodel に接続する

`@mizchi/idealis/patterns` は、ブラウザ履歴、Node.js の I/O と nextTick、オブジェクトストアの有限モデルと接続用の型を提供する。Node.js 24 で実行し、`rcheck` の実行ファイルを渡す。rcheck は idealis リポジトリのルートから `cargo build --locked --manifest-path crates/rcheck/Cargo.toml --bin rcheck` でビルドできる。

```ts
import {
  nodeFileIoBatch,
  nodeIoBatchPattern,
  recordReplay,
} from "@mizchi/idealis/patterns";

const { scenario, adapter } = nodeIoBatchPattern(
  nodeFileIoBatch(new URL(import.meta.url)),
);
const result = await recordReplay("/path/to/rcheck", scenario, adapter);
if (result.verdict === "mismatch") console.error(result.difference);
```

| パターン                                 | 利用側で用意するもの                                                                                                                        |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `browserHistoryPattern(history, routes)` | 移動を終えるまで待つ履歴操作、現在の URL と画面を読む関数、モデルの `Home`・`Catalog`・`Detail` に対応する画面                              |
| `nodeIoBatchPattern(batch)`              | 同じ I/O コールバックで登録した microtask、immediate、timer の実行順。ファイル I/O を使う `nodeFileIoBatch` も使える                        |
| `nodeNextTickPattern(batch)`             | I/O と microtask から登録した nextTick の実行順。ファイル I/O には `nodeFileNextTickBatch` を使える                                         |
| `objectStorePattern(store, testKey)`     | GET・LIST・上書き・削除を使えるストアと、最初は存在しない検査専用キー。S3 互換クライアントは `s3ObjectStore(client, bucket)` を通して渡せる |

パターンは同梱モデルの絶対パスを返す。自分のモデルを使う場合は最後の引数にパスを指定する。`recordReplay` は各操作がモデルで有効か確認したあとで実装を呼び、最初の差分までを記録する。

表示などの導出値を照合するときは、シナリオに `derive: ["screen"]` を追加し、アダプターの `project(state, derived)` で `derived.screen` を読む。たとえば一つの画面は `[["SignIn"]]` として渡る。初期状態と各操作の直後で評価し、指定した導出名は再生用 ITF にも保存する。[認証付き履歴の照合](../tests/model-patterns/patterns.test.ts)

`toReplayItf(result.recording)` を JSON として保存できる。保存したデータを `fromReplayItf` で読み、`replayRecording(rcheck, recording, projection)` に渡すと、実環境に接続せず再照合できる。標準の比較関数は `browserScreenProjection(routes)`、`ioBatchProjection`、`nextTickProjection`、`objectStoreProjection`。

`commands: []` のシナリオでは環境への操作を呼び出さず、初期観測を一件照合する。導出値を含む観測も ITF に保存でき、差分の位置はステップ0になる。[ブラウザの初期表示の試験](../tests/model-patterns/bootstrap.test.ts)

進行性の違反を実装で観測する場合は、`repeatedTemporalScenario(rcheck, model, question, repetitions)` で循環反例を操作列にし、`recordReplay` に渡す。停止だけの循環は、`repetitions = 0` で循環の入口までを再生できる。操作のある循環は有限回の繰り返しとして観測する。[重み付きセマフォの再生](../examples/model-patterns/semaphore/README.md#重み付きの要求を追い越すとどうなるか)
