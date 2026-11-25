# FizzBee

FizzBee v0.5.4 の `fizz` を使う。

```sh
FIZZBEE=/path/to/fizz bash evals/experiments/surface-languages/fizzbee/check.sh
```

- permissions は前提を満たす設定だけを初期状態として列挙する。権限に応じた開閉を調べ、秘密文書を開く例と未承認者が秘密文書を開けないことを検査する。
- bank は2口座の全残高と範囲内の入出金を検査する。
- order の状態はタグと payload の組で表現する。発送済みなら payload が変化しないことを比較する。直和の型による排除は動的な値の比較で代用した。
- sessions は同時に最大2人、ロックは最大1人。ロックしたままログアウトする反例は `exists` で確かめる。
- service は2人とも再申請しないため、永続的に待ち列が空になる性質と、各要求がいつか処理される性質が一致する。弱い公平性を外すと、要求後に stutter する反例を検出する。

DDD の rubric は identity 0.5（検査用の個数をソースに固定）、config 0.5（前提を初期状態の列挙で表現）、ints 1、adt 0.5（タグ付きの組で表現）、typeExclusion 0、step 1、lone 0.5（動的な値または None）。CLI の parser は JSON AST を生成し、検査は有限のモデルを探索する。活性は service の公平性あり・なしで実測する。

実装への接続では、[公式の MBT 手順](https://fizzbee.io/testing/tutorials/quick-start/)に従ってアダプターを実装すれば、モデルの遷移と Go の実装を照合できる。この比較では MBT の実装照合を実行していない。生成対象はテストとアダプターのひな型なので、実装コードの生成と不変条件の強制は 0 とした。
