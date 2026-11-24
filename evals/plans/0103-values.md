# 0103: 値の種類（整数・bool・record）

[0101の評価](../iterations/0101-checker-registry.md)で、DDDの道具として一番の障害は値の種類だった。整数の状態・bool・値オブジェクトがないので、金額や在庫数のような普通のドメインを書けない。mizchiと相談し、整数・bool・recordを足す。列とmapは範囲外。

## 意味論

atomのsort（sig・enum・和）とは別に、**値の型**を置く。値はatomではなく、構造で等しい。

- **`Int`**: 数学の整数。scopeの`Int = lo..hi`は、探索で数え上げる範囲（初期状態、actionの引数、量化）。遷移が範囲の外の値を作ったら、その状態は探索の外で、その先を探さない。範囲の外へ出た探索は完全ではないので、witnessで決まらないcheckは未確定にする（[0096](../iterations/0096-quint-overflow.md)の規則）。
- **`Bool`**: `true`と`false`。Bool型の式は、そのまま論理式として書ける（`a.active`は`a.active = true`）。
- **`record Money { amount: Int, currency: Currency }`**: fieldは値の型かatomのsort。値は`Money { amount: 1, currency: JPY }`、fieldは`m.amount`。fieldがすべて等しいとき等しい。
- **値を持つ関係**: `var count: one Int`（単独の値）、`var balance: Account -> one Int`（属性）、`var price: Item -> lone Money`。値の型への関係の多重度は`one`と`lone`だけ（値の集合はv1にない）。
- **式**: Int型の式どうしの`+`・`-`・`*`は算術、`<`・`<=`・`>`・`>=`は比較、`=`・`!=`はすべての値の型。`#`はIntを返す。割り算はない。`lone`の属性が空の値の算術は型エラーではなく、部分的な演算のエラーとして報告する（0099の意味論）。

## 検査器

- 参照評価器と明示状態（TLA+の写し方: Intは整数、Boolは`BOOLEAN`、recordはTLA+のrecord）はすべてに対応する。
- SAT（Alloyの写し方）はBoolに対応する。Intとrecordは、rcheckの`supports`で理由を付けて対応しないと宣言する（Alloyの整数はbit幅で回り、recordの値の意味を写す方法を決めていない）。

## 正しさ（結果を見る前に固定）

- 回帰テスト: 口座（`var balance: Account -> one Int`、`deposit(a, n: Int)`、`withdraw`は残高を確かめる、`invariant NonNegative`）、bool（`var open: Door -> one Bool`）、record（`Money`と通貨のenum、同じ通貨どうしの加算）。参照評価器と明示状態の判定が、結果の前に書いた期待と一致すること。範囲の外へ出るモデルは、未確定になること（成り立つと答えない）。
- 拒否のテスト: 値の集合、Intと集合の混在、存在しないfield、recordの値の型の不一致。
- 差分テスト: 生成器に値の型を加え（Int・Boolの変数・属性を確率30%、recordを15%）、dev seed 7000〜7299、確認 seed 8000〜8299で、参照評価器と明示状態の判定の食い違いが0件。SATは対応するモデルだけ比べる。
- 既存のcorpusと0097〜0100の回帰がすべて通ること。

## 範囲外

- 値の集合、列・map、割り当てない`lone`の既定値、Alloyでの整数とrecord。
