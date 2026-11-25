# Context Mapper CML

[共通仕様](../SPEC.md)の5題材を、集約、エンティティ、関連、操作宣言とライフサイクルで記述した。Context Mapper CLI 6.12.0 の `validate` で5件ともエラーなし。検査対象は CML の宣言。初期値、データに依存する操作ガード、更新、到達可能性、活性のうち検証できないものは各モデルに記録した。

```sh
CONTEXT_MAPPER=/path/to/context-mapper-cli-6.12.0/bin/cm ./check.sh
```

`CONTEXT_MAPPER` を省略すると PATH 上の `cm` を使う。CLI には Java が必要。macOS arm64 の検査では Java 21 を `JAVA_HOME` で指定した。構文エラーを入れた対照では、CLI 6.12.0 がエラーを出力したうえで終了コード 0 を返し、`check.sh` は終了コード 1 を返した。

| 項目          |  値 | 根拠                                                                                                                               |
| ------------- | --: | ---------------------------------------------------------------------------------------------------------------------------------- |
| identity      | 0.5 | `Entity` と `key` で識別子を表現。検査時の個体数を指定する経路はない。                                                             |
| config        | 0.5 | スカラーの `not changeable` と関連の `notEmpty` は宣言できる。集合の固定、権限設定の量化された前提、設定の列挙はできない。         |
| ints          |   1 | 残高と操作の引数に `BigInteger` を指定できる。                                                                                     |
| adt           | 0.5 | ライフサイクルの列挙型と、別フィールドのペイロードで近似する。タグごとの型と全域の分岐は表せない。                                 |
| typeExclusion | 0.5 | 列挙型で注文のタグの併存を排除できる。タグとペイロードの組み合わせは排除できない。                                                 |
| step          | 0.5 | 注文とサービスのライフサイクルを宣言できる。銀行や権限のような値に依存するガードと更新は操作シグネチャとコメントに留まる。         |
| lone          |   1 | セッションのロックは単一の `nullable` な型付き関連で表せる。                                                                       |
| tree          |   1 | Xtext のモデルをスタンドアロンライブラリで取得できる。                                                                             |
| liveness      |   0 | ライフサイクルは宣言できる。弱公平性や活性を検査する機能はこの経路にない。                                                         |
| codegen       | 0.5 | 公式の JDL テンプレートと JHipster の経路で Java のサービス雛形を生成できる。今回のモデルからの生成は未実行。                      |
| conformance   | 0.5 | 公式 ArchUnit 拡張は Java コードの型やフィールドとモデルの構造を部分照合できる。今回の実験では操作の例やトレースを照合していない。 |
| enforcement   |   0 | 公式の生成経路は CML の操作を生成対象に含めない。今回の業務不変条件の強制に使えた生成物はない。                                    |

`verification` は空。検査結果の種類を返す idealis IR adapter もない。上表はモデルと公開仕様に基づく評価で、CLI が不変条件や到達可能性を検査した結果は含まない。

[戦術的 DDD 構文](https://contextmapper.org/docs/tactic-ddd/) · [ライフサイクル](https://contextmapper.org/docs/aggregate/) · [ライブラリ](https://contextmapper.org/docs/library/) · [JHipster 連携](https://contextmapper.org/docs/jhipster-microservice-generation/) · [ArchUnit 連携](https://contextmapper.org/docs/architecture-validation-with-archunit/)
