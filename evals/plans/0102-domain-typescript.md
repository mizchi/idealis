# 0102: ドメインのモデルからTypeScriptの型を生成し、実装を照合する

mizchiの方針（DDD）: ドメイン記述言語としてドメインをモデル化し、各検査器とシームレスに繋げる。[0101](0101-checker-registry.md)の共通の枠に、実装の照合を検査器の1つとして載せる。 ドメインをモデルにし、そこから各言語の型定義を生成し、各言語で実装して、実装がモデルに従うかを検査する。最初の言語はTypeScript。値オブジェクト（record）は後で足す。

## 生成する型（`rmodel::typescript(model)`）

| モデル                    | TypeScript                                                               |
| ------------------------- | ------------------------------------------------------------------------ |
| sig（entity）             | branded string `type User = string & { readonly __sort: "User" }`        |
| enum                      | 文字列のunion `type Status = "Matched" \| "Mismatch"`                    |
| 和のsort                  | 子の型のunion `type Party = Customer \| Staff`                           |
| `rel`（設定）             | `interface Config`のfield                                                |
| `var`（状態）             | `interface State`のfield                                                 |
| 集合（`set`・`some`）     | `ReadonlySet<T>`                                                         |
| 集合（`lone`・`one`）     | `T \| undefined`・`T`                                                    |
| 二項関係（`set`・`some`） | `ReadonlyMap<A, ReadonlySet<B>>`                                         |
| 二項関係（`lone`・`one`） | `ReadonlyMap<A, B>`（`lone`は鍵がないことがある）                        |
| action（command）         | 判別union `{ readonly kind: "read"; readonly u: User; readonly d: Doc }` |

実装が満たすinterface: `create(config: Config): Domain`、`Domain.state: State`、`Domain.apply(command: Command): boolean`（受け付けたか）。atomの値と状態をJSONへ写す関数も生成する。

## 照合

- 意味論は参照評価器だけに置く。TypeScriptの実装は、JSON Linesで話すdriver（`reset(config)`・`apply(command)`、答えは受け付けたかと状態）として、Rustの照合器から動かす。Rust・Goの実装も同じprotocolのdriverで照合できる。
- 照合器は、factを満たす設定ごとに、実装の初期状態がモデルの初期状態（型・多重度・`init`）であることを確かめ、実装が到達する状態を幅優先で辿る。各状態・各actionの引数の組で、モデルの条件が成り立つかと実装が受け付けたかが一致し、受け付けたときの次の状態がモデルの次の状態と一致すること。実装の状態は、初期状態からのcommandの列を新しいインスタンスで再生して作る。
- 結果は、一致・不一致（最短のcommandの列と、期待と実測）・未確定（予算切れ）を区別する。

## 正しさ（結果を見る前に固定）

- corpusの`permissions.model`と`sessions.model`をTypeScriptで実装し、照合が一致すること。
- 人工故障: `read`で付与を確かめない、`close`で文書を外さない、`login`で定員を確かめない、の3件を照合が不一致として検出し、最短のcommandの列を示すこと。
- 生成した型が`tsc`の型検査を通り、生成物が古くないこと（再生成との一致）。

## 範囲外

- Rust・Goの型生成、値オブジェクト・整数の状態、実装の状態の一部だけを観測すること。
