# Idealis の仕様を読む

Idealis では、状態を変える操作と、守るべき性質をソースに書きます。実装の処理を読む前に、モデルから許される操作順と準備するテストを確認できます。

**最初のモデルは [payment.ideal](../examples/model-patterns/payment-data/payment.ideal) です。** 支払申請を1件受け付けるモデルで、`data` が扱う値、`var` が状態、`behavior` が操作、`invariant` と `reachable` が検査項目です。支払額と手数料は同じ自然数を使いますが、型名で別々に扱います。手数料と支払額の関係は、受け付ける操作の条件に書いてあります。

```sh
cargo build --locked --manifest-path crates/idealis-check/Cargo.toml --bin idealis-check
checker=crates/idealis-check/target/debug/idealis-check
model=examples/model-patterns/payment-data/payment.ideal

"$checker" "$model" --checker reference --question FeeWithinAmount --expect holds
"$checker" witness "$model" Submitted
"$checker" generate "$model" typescript
```

`--question FeeWithinAmount` は、受け付けた申請で手数料が支払額を超えないかを調べます。`witness` は実際に申請を受け付ける操作列を返します。`generate` では、アプリやアダプタが使う操作と状態の型を生成します。

次に [payment-refined.ideal](../examples/model-patterns/payment-data/payment-refined.ideal) を読むと、同じ手数料の条件を申請型の `where` に書けます。受付操作は、入力がすでに有効な申請であることを前提にします。`pbt-data-cases` で、生成したデコーダーに渡せる境界値を確認できます。

```sh
model=examples/model-patterns/payment-data/payment-refined.ideal
"$checker" "$model" --checker explicit --question FeeWithinAmount --expect holds
"$checker" "$model" --checker rprob --question FeeWithinAmount --expect holds
"$checker" refinements "$model" --type Request
"$checker" generate "$model" pbt-data-cases
```

## このリポジトリの構成を知る

| 読みたいこと                                         | 最初に開くファイル                                                                                                                         |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| TLA+ のソースをどの crate が読み、何を渡すか         | [処理系の辞書](toolchain.ideal)。`input`、`output`、`code` をたどる。                                                                      |
| Idealis が「完全」「有界」「未確定」をどう区別するか | [Idealis の用語と規則](idealis.pkl)。規則が参照する検査は `claims` にある。                                                                |
| 用語と規則を別のドメインにつなぐ方法                 | [貸出と会員管理](../examples/model-patterns/library-membership/README.md)。辞書、資格を読むポート、通知遅延の反例がある。                  |
| 一つのモデルを Alloy/SAT と TLA+ でどう検査するか    | [複数の手法で検査する設計](../crates/idealis-model/design.md#一つのドメインモデルを複数の手法で検査する)。共通仕様と検査計画の役割を示す。 |
| `.ideal` の状態と操作を基礎から読む                  | [入門ガイド](../docs/ideal-tutorial.md)。貸出と返却を題材に反例を読む。                                                                    |
