# サーキットブレーカーの回復試験

通常状態で受け付けた要求には世代を記録する。失敗したら遮断し、時間の経過で半開状態へ進む。半開中は一つの試験要求だけを許可する。試験要求が成功したら通常状態に戻り、失敗したら次の世代で遮断する。通常状態で出した古い要求の応答が届いても、その応答が現在のブレーカーの状態を変えることはない。[モデル](half-open.ideal)

リポジトリのルートで実行する。

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml
checker=crates/idealis-check/target/debug/idealis-check
model=examples/model-patterns/circuit-breaker/half-open.ideal

"$checker" "$model" --checker explicit --question TrialEventuallyResolves --expect holds
"$checker" witness "$model" TrialWithOlderPending --checker rprob
"$checker" witness-step "$model" attempt 'before(phase) = HalfOpen and some before(probe)'
just build
node --test dist/tests/model-patterns/circuit-breaker.test.js
```

Node.js の試験は下流 Promise を操作列どおりに完了させ、受付結果、保留中の要求の世代、試験の所有者、現在の状態を照合する。

- **試験中に別の受付が来る。** 正常版は拒否し、試験を二つ通す故障は5手目で拒否すべき要求を送った。
- **試験中に古い成功が届く。** 正常版は試験を継続し、古い成功で通常状態へ戻す故障は6手目で差分になった。現行の試験が成功すれば通常状態に戻り、失敗すれば次の世代で遮断することも同じ前置きから照合した。

`TrialEventuallyResolves` は個々の要求・結果の完了通知に弱公平性を置く。公平性を外すと、試験要求が完了しない反例が出る。モデルは要求 ID 三つ、世代三つを扱う。待機時間は `elapsed` に抽象化し、最後の世代からさらに遮断する遷移は範囲外。[Node.js の試験](../../../tests/model-patterns/circuit-breaker.test.ts)
