# セマフォの予約をキャンセルする

許可が一つある。待機列の先頭への予約と、取得を知らせる Promise の完了は別の操作。予約をキャンセルした時点で次の待機者に許可を渡せる。古い通知が後で届いた場合も、次の予約を保つ。[モデル](cancel-reservation.model)

| モデルの状態         | Node.js で読む値                                 |
| -------------------- | ------------------------------------------------ |
| `waiting`            | 予約を待つ要求の順序                             |
| `reserved` と `held` | 通知前の予約、通知後に許可を持つ要求             |
| `callbacks`          | 配送境界に残っている取得通知                     |
| `cancelled`          | AbortSignal によってキャンセルで完了した Promise |

リポジトリのルートで実行する。

```sh
cargo build --locked --manifest-path crates/rcheck/Cargo.toml
rcheck=crates/rcheck/target/debug/rcheck
model=examples/model-patterns/semaphore/cancel-reservation.model

"$rcheck" "$model" --checker explicit --question WaiterSettles --expect holds
"$rcheck" witness-step "$model" deliver 'client in before(cancelled) and some before(reserved)'
just build
node --test dist/tests/model-patterns/semaphore.test.js
```

モデルが返す7手は、二人を待機列に入れ、許可を返却し、一人目に予約してキャンセルし、二人目に再予約してから一人目の通知を届ける。Node.js では通知の配送順を制御し、各通知を microtask として実行する。続けて二人目に通知すると、正常版では一人目がキャンセル、二人目が取得で完了する。予約を返さない故障は5手目、古い通知が予約を上書きする故障は7手目で差分になる。[接続試験](../../../tests/model-patterns/semaphore.test.ts)

`WaiterSettles` は返却・予約・通知の弱公平性を仮定し、待機中または予約済みの要求が取得かキャンセルへ進むことを検査する。公平性を外すと、待機したまま進まない実行がある。モデルの範囲は許可一つと待機者二人。

## 重み付きの要求を追い越すとどうなるか

[重み付きモデル](weighted-fifo.model)では、二つの許可を個体として扱う。重い要求は二つ、軽い要求は一つ必要。FIFO は先頭に必要な数がそろうまで後続を待たせる。対照では `grant` のガードを `client = at(waiting, 0)` から `client in elements(waiting)` に変え、空きが足りる待機者をスケジューラが選べるようにする。

両方で、各許可の所有者は高々一人で、取得した数も要求の重みと一致した。弱公平性では FIFO の先頭がやがて取得できる一方、追い越しを許すと、軽い要求が取得と返却を繰り返して重い要求が待ち続ける循環反例が出た。重い要求の取得も何度も有効になるので、引数ごとの取得に強公平性を置いた対照では進行性が成立した。

`@mizchi/idealis/patterns` から `repeatedTemporalScenario(rcheck, model, "HeadGetsCapacity", 2)` を呼ぶと、循環反例を有限の操作列に展開できる。Node.js の照合では循環を二周させ、入口へ戻るたびに重い要求がまだ待っていることを確認した。FIFO の実装では同じ操作列の最初の追い越しが観測差になり、正規の取得経路では重い要求に二つの許可を渡した。[接続試験](../../../tests/model-patterns/weighted-semaphore.test.ts)

### 最初に収まる要求を選ぶ版

[最初に収まる要求のモデル](weighted-first-fit.model)では、初期状態で二人の軽い要求が許可を一つずつ持つ。取得できるのは、待機列の中で現在の空きに収まる最初の要求。候補より前に収まる要求がないことを、有限 `Nat` の添字で表す。

軽い要求が交互に許可を返して再取得すると、空きは一つ以下のままになる。重い要求は取得操作が有効になる機会を得られないため、取得に引数ごとの強公平性を置いても待ち続ける。先頭だけに取得させる FIFO 版では、後続が再取得できず空きがそろう。[接続試験](../../../tests/model-patterns/weighted-semaphore.test.ts)
