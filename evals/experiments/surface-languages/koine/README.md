# Koine 0.251.0

[5題材の仕様](../SPEC.md)を Koine の aggregate と command で記述した。生成先での guard、不変条件、具体的な実行列を `check.sh` で検査する。[Koine の command・状態機械リファレンス](https://atypical-consulting.github.io/Koine/reference/commands-events-state/)

`bash check.sh` は C# と TypeScript を一時ディレクトリへ生成し、TypeScript の型検査と C# の実行を行う。Koine は `KOINE` で指定できる。実測環境は Koine 0.251.0、.NET SDK 11.0.100。Homebrew で .NET 11 のみが入った環境では、.NET 10 を対象とした Koine shim の起動に `DOTNET_ROOT` と `DOTNET_ROLL_FORWARD=Major` を指定した。

| 題材        | 実行した観測                                                                                                | 表現の制約                                                                                                              |
| ----------- | ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| permissions | 不正な設定と権限のない操作を拒否。2文書を開いた後に1文書だけ閉じ、別の設定で cleared 利用者が秘密文書を開く | 1 aggregate は利用者と文書の1組。組をまたいで設定が一致することは呼び出し側の責任。到達可能性の構文なし                 |
| bank        | 1手で残高3、2口座の残高の合計8への到達。上限超過と引き落とし超過を拒否                                      | `Int` は C# では機械整数。足し算の前に上限を検査                                                                        |
| order       | 出荷済みの取り消しを拒否し、追跡値を保持。1手で Fraud による取り消しへ到達                                  | 状態タグと nullable payload、不変条件で直和を近似。二状態の述語は書けないため、終端の状態機械と command の guard で表現 |
| sessions    | 管理者集合が空の設定を拒否。Full の実行列、容量ガード、`LockedIsActive` の反例を実行。unlock で再び空になる | active は3人分の Bool。空の optional を持つ読み取り専用フィールド経由でロックを解除。到達可能性の構文なし               |
| service     | 2人の request と一方の serve を実行。早すぎる serve と再 request を拒否                                     | client ごとの状態と disjoint 不変条件。活性と weak fairness の記述・検査機能なし                                        |

## 採点根拠

| 項目          |  値 | 根拠                                                                                                                                                  |
| ------------- | --: | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| identity      | 0.5 | nominal な ID 宣言はある。有限 scope を検査器に渡す機能はなく、sessions は3ユーザーを明示                                                             |
| config        | 0.5 | 読み取り専用フィールドと constructor の不変条件。permissions の全体の関係は各組への投影で代用                                                         |
| join          | 0.5 | 入れ子の `any` で role を照合                                                                                                                         |
| ints          | 0.5 | 対象言語の整数型とガードで有限部分を再現                                                                                                              |
| adt           | 0.5 | smart enum と optional な別フィールドの組合せ                                                                                                         |
| typeExclusion |   0 | タグと payload の整合性を runtime invariant で拒否。payload 付き直和として型で表現する機能なし                                                        |
| step          | 0.5 | ShippedStays は終端指定・ガード・payload の代入箇所で表現                                                                                             |
| count         | 0.5 | DSL にリストの `count` はあるが、sessions の更新に使った固定スロットの数は Bool から計算                                                              |
| lone          |   1 | ロック対象の型は `UserSlot?`                                                                                                                          |
| liveness      |   0 | weak fairness の下での時間的性質を検査できない                                                                                                        |
| tree          |   1 | コンパイラの `KoineCompiler.Parse` が型付きの宣言ノードを含む `KoineModel` を公開                                                                     |
| verification  |   — | toolchain の静的検査と生成コードの例を実行。全探索・有界検査・帰納法・証明は対象外                                                                    |
| codegen       |   1 | 5モデルの C# と TypeScript を生成し、両方のコンパイルを確認                                                                                           |
| conformance   |   0 | 外部実装に例や trace を適用する機能は今回確認できず。上の例は C# の生成コードに手書きで適用                                                           |
| enforcement   |   1 | C# は private setter と、生成した constructor・command による不変条件の検査を実行。TypeScript は可変フィールドが公開され、代入で command を迂回できる |

[Koine の型リファレンス](https://atypical-consulting.github.io/Koine/reference/contexts-and-types/)・[式リファレンス](https://atypical-consulting.github.io/Koine/reference/expressions/)・[コンパイラ実装](https://github.com/Atypical-Consulting/Koine/blob/main/src/Koine.Compiler/Services/KoineCompiler.cs)
