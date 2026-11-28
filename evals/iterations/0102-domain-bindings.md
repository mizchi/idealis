# 0102: ドメインのモデルから型を生成し、実装を照合する

[plan](../plans/0102-domain-typescript.md)（改訂を含む）のとおり、モデルからRustのtrait・TypeScriptの型・JSON Schemaを生成し、各言語の実装が参照評価器の意味に従うかを照合した。

```sh
idealis-check generate fixtures/rmodel/permissions.model rust        # trait Permissions と型
idealis-check generate fixtures/rmodel/permissions.model typescript  # 型、Domain、serve
idealis-check generate fixtures/rmodel/permissions.model schema      # Config・State・Command
```

- **Rust**: actionをmethodに持つtrait（`fn read(&mut self, u: User, d: Doc) -> bool`）、`new(config)`・`state()`。atomは`User(u8)`のnewtype、enumはRustのenum、和のsortは子のenum、recordはstruct、集合と関係は`BTreeSet`・`BTreeMap`。`conformance::<Impl>(budget)`が同じプロセスで照合する。
- **TypeScript**: atomはbranded string、enumは文字列のunion、集合と関係は`ReadonlySet`・`ReadonlyMap`、commandは判別union。実装は`create(config): Domain`（`state`と`apply(command): boolean`）を書き、`serve(create)`がJSON Linesのdriverになる。
- **JSON Schema**（draft 2020-12）: sortのatomの集合、enum、多重度（`one`・`lone`・`some`）を制約にする。照合器は実装の答えをまずschemaで検査し、合わなければinvalidにする。
- **照合**（`idealis_check::conform`）: 参照評価器を1歩ずつ進める`reference::Machine`の上で、設定ごとに、実装の初期状態がモデルの初期状態であること、到達する各状態で各action instanceの受け付けと次の状態が一致することを幅優先で確かめる。実装は、初期状態からのcommandの列を新しいインスタンスで再生して動かす。

## 計画からの差（結果の前）

- JSONの形を契約として決めた: atomは名前（`"User0"`、enumの`"JPY"`）、BoolはJSONのboolean、Intは整数、recordはobject、集合は配列、二項関係は定義域のすべてのatomを鍵にしたobject。比較では配列を集合として並べ替える。
- 照合器のschemaの検査は、生成するキーワードだけを扱う自前のvalidator（知らないキーワードはエラー）にした。独立性はajv 8.20.0（dev依存に追加）と同じfixtureを検査して確かめる。
- TypeScriptのdriverは、既存のテストと同じく`just build`で`dist/`に出したものを動かす（`just idealis-check-typescript`、CIに追加）。

## 正しさ

| plan の条件                                                                                                                        | 結果                                                                          |
| ---------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| permissions・sessionsをTypeScriptで実装し、照合が一致                                                                              | 一致                                                                          |
| permissions・sessions・口座をRustで実装し、照合が一致                                                                              | 一致。permissionsは2817設定・11184状態・89472遷移（driverの呼び出し296705回） |
| 人工故障3件（`read`で付与を確かめない、`close`で文書を外さない、`login`で定員を確かめない）を不一致として、最短のcommandの列で検出 | Rust・TypeScriptとも検出。列の長さは1・2・3                                   |
| 生成した型が`tsc`の型検査を通る、生成物が古くない                                                                                  | 通る。Rust・TypeScript・schema・到達できる状態の再生成と一致                  |
| schemaが照合で得た全状態を受け付け、多重度を破る3件を拒む                                                                          | 自前のvalidatorとajvの両方で、全状態（16・28・25）を受け付け、3件を拒んだ     |

上限の扱い: 設定の上限で止めた照合はbounded、呼び出しの予算切れは未確定と答え、一致とは言わない（テストで確認）。

人工故障の検出は、この3件を検出できることを示すだけで、未知の不具合を見つける力の評価ではない。

## 範囲外のまま

- Goの型の生成。実装の状態の一部だけを観測すること（今は状態をすべて返す）。
- 値の集合、列・map。recordの値を鍵にする関係。
- 再生の費用: 遷移ごとに初期状態から再生するので、呼び出しは遷移数の数倍になる（permissionsで約3.3倍）。
