# 同じSafetyProgramを再利用する

準備済みSafetyProgramは、同じ仕様に異なる探索予算・optionsを渡すときに、IR転送・静的検証・定数索引の構築を償却する。Rustとnative/Wasm protocolに対応する。新しいモデル、異なる定数設定、実装traceの照合には別の入力・APIが必要で、既存のfinite model handleと区別する。

## TSとブラウザ

```ts
import { prepareSafetyProgram } from "@mizchi/idealis/portable";

const prepared = prepareSafetyProgram(core, program);
try {
  const results = prepared.checkBatch([
    { bounds: { maxDepth: 2, maxStates: 100, maxWork: 1000 } },
    { bounds: { maxDepth: 10, maxStates: 1000, maxWork: 100000 } },
  ]);
  for (const item of results) {
    if (item.kind === "error") throw new Error(item.message);
    console.log(item.result.status, item.result.reason);
  }
} finally {
  prepared.release();
}
```

`check(bounds, options?)`は通常のSafetyResultを返す。`checkBatch(jobs)`は1〜256件を指定順に実行し、各jobの`{kind:"safety", result}`または`{kind:"error", code, message}`を同じ位置に返す。`safety`は問い合わせの結果がある意味で、`complete`の判定とは区別する。`bounded`・`empty`・違反・エラーをそれぞれ確認する。

各jobの予算は独立している。batch全体の時間・workを制限したい場合は、呼出し側で各jobへ予算を配分する。1件の実行エラーで後続jobを省略しない。不正なJSON構造、未知のhandle、空・過大なbatchは外側のprotocol errorになる。

prepareは入力のsnapshotを保持し、呼出し側のJS objectを書き換えても反映しない。helperのreleaseは複数回呼べるが、release後の検査は拒否する。handleは作成したcore/Engine内でのみ使い、永続IDやモデルの合格を表さない。

## Rust

```rust
let prepared: rtlc::PreparedSafetyProgram = program.prepare()?;
let result = prepared.check(bounds)?;
let other = prepared.check_with_options(other_bounds, options)?;
let coverage = prepared.check_with_coverage(bounds)?;
```

`SafetyProgram::prepare(self)`はprogramを所有して静的な構造を検証する。探索、ASSUME、定数invariant、Data値の評価はcheckで行うため、prepare成功は安全性や実行成功を保証しない。未選択の値を先に評価せず、失敗を成功した定数としてcacheしない。

準備済みの不変データは複数のRust callerで共有できる。Arena・constant Scalar cache・visited・graph・symmetryの代表・work・coverageはcheckごとに新規となり、過去の探索結果を引き継がない。coverage付きの検査にも同じ制約が適用される。

## Protocol

| request kind                  | 入力                                   | 応答                                |
| ----------------------------- | -------------------------------------- | ----------------------------------- |
| `prepare-safety`              | `program`                              | `safety-prepared`と`handle`         |
| `check-prepared-safety`       | `handle`, `bounds`, optional `options` | `safety`                            |
| `check-prepared-safety-batch` | `handle`, `jobs`                       | `safety-batch`と位置を保つ`results` |
| `release-safety`              | `handle`                               | `released`                          |

外側のprotocol versionは1を維持する。既定IR v1と明示v2、未対応入力の拒否、論理workを変更しない。batchはjobを逐次実行し、nativeの各job内では既存のworkers optionを使える。Wasmの探索は逐次である。
