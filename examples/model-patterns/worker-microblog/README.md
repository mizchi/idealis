# Workers の短文投稿サービス

投稿 Worker が確定済みの投稿を Queue に送り、consumer Worker がフォロワーのタイムラインに反映する。Cloudflare Queues の再配信と順序の非保証を含む有限モデルを使う。[配信保証](https://developers.cloudflare.com/queues/reference/delivery-guarantees/) · [順序とバッチ](https://developers.cloudflare.com/queues/reference/how-queues-works/) · [ACK と再試行](https://developers.cloudflare.com/queues/configuration/batching-retries/)

同じ構成を、`import`・`export model`・`instance`・`connect` を使う構文案と、現行のプロジェクト定義の両方で書いた。[構文案](composition.md) · [実行できるプロジェクト](model.project.json)

| 部品                              | 所有する状態                       | 他の部品から読む状態         |
| --------------------------------- | ---------------------------------- | ---------------------------- |
| [ユーザーとフォロー](users.ideal) | フォロー関係                       |                              |
| [投稿](posts.ideal)               | 確定した投稿と著者                 |                              |
| [送信 relay](relay.ideal)         | Queue への送信要求                 | 確定した投稿、Queue 受理済み |
| [Queue](queue.ideal)              | 受理済み、待機中、配信中、ACK 済み | 送信要求、ACK 要求           |
| [タイムライン](timeline.ideal)    | ユーザーごとの索引行、反映済み     | 著者、フォロー関係、配信中   |
| [consumer](consumer.ideal)        | 配信 ID ごとの ACK 要求            | タイムライン反映済み、配信中 |

## ACK の呼び出し

consumer が配信中の投稿と反映済みの集合を読み、両方に該当するとき `requestAck(post, delivery)` で要求を出す。Queue は現在配信中の組に対応する要求を照合し、その配信を完了状態にする。要求は観測履歴に残す。初回の要求を発行した後、ACK が届く前に再配信へ進む経路があり、その場合は再配信用の要求が必要。論理的な投稿 ID は索引行の重複防止、配信 ID は ACK の照合に使う。

Queue の読み取り接続は送信要求と ACK 要求の二つ。フォロー関係、投稿の著者、永続化条件を持つのはアプリケーション側で、consumer だけが ACK 要求を発行できる。relay が ACK 要求を書こうとする対照は `relay_offer cannot write ackRequests` と探索前に拒否された。

`requestAck` と `ack` は別々のステップとして検査する。この分け方で直接のメソッド呼び出しに相当する流れを試した。送信と処理を一手で行う構文を設ける場合は、同期呼び出しとして別に意味を決められる。

## 検査する範囲

Author が Earlier と Later をこの順で投稿し、relay も Earlier を先に Queue に渡す。Reader が Author をフォローしたり解除したりする。各投稿の配信は初回と再配信1回に絞る。同じ ID の Queue 内の複数コピーや同時 consumer 実行は集約する。`Delivery` はアプリ内の要求の対応を検査するモデルの識別子。フォロー関係は索引への反映時と取得時に読み、`read` は1件を返す操作を表す。配信の最終到達、バッチの部分成功、DLQ、タイムラインの表示順とページングは今後の検査範囲。

初回試作は両方のユーザーを投稿者や読み手にできたが、故障版の探索が20万状態の予算内に収まらなかった。投稿者と読み手を固定して、このケースの正常経路と反例を照合した。

## 検査結果

参照評価器、TLA+ に変換する明示検査器、rprob で、宣言の制約を含む12項目の判定が一致した。

- 正常版では、反映後の ACK と配信 ID の照合が成立。初回の要求を残したまま再配信中に進め、再配信用の要求で ACK できる経路は10手。初回に正常に ACK する経路は7手。
- 先に投入した Earlier を Later が追い越して反映される経路は8手。反映後に再配信を処理する経路も8手。
- フォローを解除した後にも索引行が残る状態は到達でき、取得時に現在のフォロー関係を確認する性質は成立。

| 意図的に入れた故障                   | 最短の反例                                        |
| ------------------------------------ | ------------------------------------------------- |
| consumer から反映確認を外す          | 6手で未反映のまま要求を出し、Queue が ACK         |
| Queue から配信 ID の照合を外す       | 9手で初回向けの要求が再配信を ACK                 |
| 再配信時に新たなフォロワーを追加する | 9手で反映済みの投稿を新たなフォロワーの索引に追加 |
| 取得時のフォロー確認を外す           | 8手でフォロー解除後に古い行を取得                 |

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml
checker=crates/idealis-check/target/debug/idealis-check
model=examples/model-patterns/worker-microblog/model.project.json

"$checker" "$model" --checker reference --max-states 200000
"$checker" witness "$model" AckedAfterRedelivery --max-states 200000
"$checker" witness "$model" DelayedInitialAckRequest --max-states 200000
```

## Workers 実行への接続で確かめる条件

ローカルの Miniflare に Queue と D1 を束縛し、アプリの投稿・索引・ACK の経路を実行する。索引を確定した直後にテスト用の観測先が一度失敗し、実環境の再配信を待つ。同じメッセージの試行番号が増えること、索引行が一件のままであること、再配信で ACK することを調べる。実測した境界イベントをモデルの操作に写し、D1 から直接読んだフォロー・投稿・索引・反映済みのスナップショットを ITF で照合する。最後にフォローを外し、索引行が残っていても HTTP から返らないことを確認する。

[アプリの処理](app.ts)では、フォロー、投稿、送信の判断、索引への反映を書く。[Workers の接続](worker.ts)で `connectMicroblog` に渡すのは、D1 で実装したストアと `env.POSTS.send`。受信側は Workers が呼ぶ `queue()` からアプリの `applyPost` に入り、成功した場合に配信されたメッセージの `ack()` を呼ぶ。D1 では索引行と処理済み ID を一つの `batch()` で確定する。Queue と D1 は Miniflare が起動するローカルの workerd に束縛する。[ローカル Queue](https://developers.cloudflare.com/workers/testing/miniflare/core/queues/) · [D1 のトランザクション](https://developers.cloudflare.com/d1/worker-api/d1-database/#batch)

モデルの `connect` に対応する実装上の接続は次のとおり。

| モデルの接続                                | 実行時の接続                                                      |
| ------------------------------------------- | ----------------------------------------------------------------- |
| `queue.offered <- relay.offered`            | 投稿の存在を D1 で確認してから、binding の `send` で Queue に渡す |
| `consumer.inFlight <- queue.inFlight`       | Queue が呼ぶ `queue()` が、メッセージ本体と試行番号を渡す         |
| `consumer.applied <- timeline.applied`      | D1 の反映トランザクションの完了を待つ                             |
| `queue.ackRequests <- consumer.ackRequests` | 完了後、処理していた同じメッセージの `ack()` を呼ぶ               |

[統合テスト](../../../tests/model-patterns/worker-microblog-runtime.test.ts)では、最初の反映確定後に観測用 binding から HTTP 503 を返した。Worker が `retry()` を呼ぶと、同じ Queue メッセージ ID で試行番号1と2が観測され、試行番号2で ACK した。フォロー中の取得は HTTP 200、解除後は HTTP 404。D1 の索引行は1件のまま残った。観測したイベントをモデルの操作に写し、実測したフォロー・投稿・索引・反映済みのスナップショットを ITF にして再照合すると `matched` になった。

照合範囲は D1 から直接取得できる状態と、`send` の完了、配信コールバック、`retry`・`ack` 呼び出しの境界。Queue の内部状態はモデルで再生し、環境から取得したのはメッセージ ID と試行番号。接続は TypeScript に明示した。

```sh
pnpm install --frozen-lockfile
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml
just build
node --test dist/tests/model-patterns/worker-microblog-runtime.test.js
```

## Idealis から接続面の型を生成する

操作名と引数、モデルの値・状態の型は [model.project.json](model.project.json) から [idealis-generated.d.ts](idealis-generated.d.ts) に生成する。実行時のイベントをモデルの操作に写す `worker.ts` は `Command` と `Delivery` を `import type` で読む。`app.ts` の処理、ストアのインターフェース、Workers と D1 の bindings は手書きの実装。モデルを変更したら型宣言を再生成し、接続で使う操作名や引数を型検査する。

アプリの投稿 ID とユーザー ID は `string`。モデルでは検査範囲を有限にするために `Earlier`、`Later` などを列挙する。Worker では実測を文字列 ID のまま記録し、照合時に Idealis が値と操作の合法性を検査する。再生成した宣言とチェックイン済みの宣言が一致することはモデルパターンのテストで確認する。

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml
crates/idealis-check/target/debug/idealis-check generate \
  examples/model-patterns/worker-microblog/model.project.json typescript-types \
  > examples/model-patterns/worker-microblog/idealis-generated.d.ts
```
