# 0075-stack-stage

[0075-stack](../0075-stack/) の連鎖の表で、`if 127`、`if 128`、`case 128` が `compile 拒否` と書かれ、保存した記録には 1 GiB の thread での `exit: -6`（stderr なし）しかなかった。stderr を保存して、失敗した段を調べ直した。

条件: `evals/experiments/nesting-chains/stack-harness` の `chain` モード（v2、1 GiB の thread）を、0075-stack と同じ before / after、debug / release の 4 つの build で、`if`・`flat`・`case` の n = 3, 47, 48, 61, 127, 128 に流した。driver は `stage.ts`。各セルの終了コード、signal、stdout、stderr を `chains-<profile>-<side>.json` に保存した。harness は parse と compile の失敗に `unwrap()` を使い、`panic = "abort"` なので、parse か compile が失敗すると SIGABRT（exit -6）になる。

| セル | 4 つの build すべてでの結果 | 失敗した段 |
| --- | --- | --- |
| if 127 | SIGABRT | parse（`ParseError`、`Expression nesting exceeds 64`、`rtla::parse_module` の失敗） |
| if 128 | SIGABRT | parse（同上） |
| case 128 | SIGABRT | compile（`CompileError`、`compiled expression nesting exceeds 128`、frontend の出力検査） |
| 上記以外の 33 セル（if 3・47・48・61、flat の全 6、case 3・47・48・61・127） | 終了コード 0、`chain complete 6 7 20` | なし |

- 3 セルとも、`check` には届いていない。評価の stack の失敗でも、1 GiB での評価の失敗でもない。
- `compile 拒否` は `case 128` では正しく、`if 127` と `if 128` では誤り。この 2 つは compile の前の parse で、既知の Rust の parser の入れ子 64 の上限（括弧つき・IF 連鎖の 62 段以上）で拒否されている。
- before と after で、3 セルの失敗した段とメッセージは同じだった。
- 0075-stack の元の記録は変えていない。
