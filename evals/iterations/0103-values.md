# 0103: 値の種類（整数・bool・record）

[plan](../plans/0103-values.md)のとおり、atomのsortとは別に値の型を入れた。`Int`（scopeの`Int = lo..hi`）、`Bool`、`record`。値は構造で等しく、関係は値を`one`か`lone`だけ持つ。意味論は[docs/kernel-semantics.md](../../docs/kernel-semantics.md)に追記した。

```
enum Currency = JPY | USD
record Money { amount: Int, currency: Currency }
sig Wallet
var cash: Wallet -> one Money
action add(w: Wallet, m: Money) {
  m.currency = w.cash.currency
  w.cash.amount + m.amount <= 3
  cash' = cash - w -> w.cash + w -> Money { currency: w.cash.currency, amount: w.cash.amount + m.amount }
}
```

## 計画からの差（結果の前）

- 算術と整数の比較の被演算子を、静的に値が1つに決まる式に限った。planは「`lone`の属性が空の値の算術は、部分的な演算のエラーとして報告する」としていたが、型エラーにした。評価器にエラーの経路を作らずに済み、利用者にも型の時点で分かる。
- Boolは、組み込みのenum（`true`・`false`）にした。既存のenumの仕組みで、AlloyのSATも対応する。
- 両辺がIntの`+`・`-`・`*`を算術として読む。型検査の後に式を書き換える。
- 組み込みのcheck `Scope`を足した。状態が整数の範囲の外へ出たら違反で、明示状態は否定の答えをそのとき未確定にする。

## 結果を見た後に直したこと

- **TLA+の写し方**: 回帰テストの口座のモデルで、rtlcが`Invalid safety variable read`を返した（差分テストで未分類だった2つ目のrtlcのエラー）。最小化すると、束縛変数が2つの集合の内包（`{<<x, y>> : x \in S, y \in T}`）で、`T`に別の内包があると起きた（[#112](https://github.com/mizchi/idealis/issues/112)と同じ系統）。直積・算術・recordを、束縛変数が1つの内包の`UNION`に写すよう変えた。値を持たないモデルの出力も変わる。TLC 1.8.0とAlloy 6.2.0の記録をし直し、corpusの20 checkの判定は変わらなかった。0100の差分テストでrtlcがエラーになった47モデルは、新しい写し方で47とも、エラーにならなかった。
- **parser**: 生成したモデルで、`(n + 0) < n`の括弧の中をBoolの式として読んでいた。括弧の後に`<`・`<=`・`>`・`>=`・`*`が続く場合を加え、回帰テストを足した（直す前に失敗することを確かめた）。
- **生成器**: recordのfield `n`と、actionの引数`n`の名前が衝突した。引数を`k`にした。

## 回帰テスト（結果の前に書いた期待）

| モデル                                      | check                         | 参照評価器・明示状態                        |
| ------------------------------------------- | ----------------------------- | ------------------------------------------- |
| 口座（`Account -> one Int`、`Int = -4..4`） | `NonNegative`                 | 成り立つ                                    |
|                                             | `Rich`（残高3以上）           | 到達できる（深さ1）                         |
| 引き出しで残高を確かめない口座              | `NonNegative`                 | 破れる（深さ1）                             |
| 預け入れの上限を外した口座                  | `NonNegative`・`Declarations` | 未確定（範囲の外へ出る）                    |
| 扉（`Door -> one Bool`）                    | `AllClosed`・`BothOpen`       | 破れる（深さ1）・到達できる（深さ2）        |
| 財布（`Money`のrecord）                     | `OnlyYen`・`Three`・`Dollars` | 成り立つ・到達できる（深さ1）・到達できない |

拒否: 値の集合、空になりうる値の算術（`lone`、`A.b`）、Intとatomの和、recordの未知・欠けたfieldとfieldの型、Intの量化、Boolでない式を論理式に使うこと、scopeのないInt。rcheckは、整数とrecordのモデルでSATを対応しないと宣言し、範囲の外へ出るモデルで参照評価器と明示状態の両方が未確定と答える。

## 差分テスト（check数、`Declarations`を含む）

| 分類                                                          | dev（7000〜7299） | 確認（8000〜8299） |
| ------------------------------------------------------------- | ----------------: | -----------------: |
| 比べたseed（全300）                                           |               300 |                300 |
| 値を持つモデル                                                |               120 |                120 |
| 値を持つモデルのcheckで、参照評価器の判定が決まり空でないもの |               177 |                206 |
| 一致（空・未確定を除く）                                      |               468 |                464 |
| 空で一致                                                      |               270 |                267 |
| rtlcのエラー                                                  |                 0 |                  0 |
| rtlcの予算切れ                                                |               211 |                213 |
| 明示状態の未確定（範囲の外）                                  |                13 |                 11 |
| 参照評価器の予算切れ・未確定                                  |                96 |                 86 |
| crash                                                         |                 0 |                  0 |
| 判定の食い違い                                                |                 0 |                  0 |

[dev](../runs/0103-values-development/results.jsonl)・[確認](../runs/0103-values-confirmation/results.jsonl)。値を持つモデルではSATを比べない（`unsupported`）。

- rtlcのエラーは、0100の60・81件から0件になった。生成器も変わったので、写し方の効果だけの数ではない。上の47モデルの再実行は、写し方だけを変えた比較。
- 予算切れは増えた。値を持つモデルでは、rtlcが360 check中157・139件、参照評価器が72・52件。値を持たないモデルでも、rtlcは540 check中54・74件で、0100（897 check中67・53件）より割合が高い。束縛変数が1つの写し方の費用は、同じモデルで測っていない。

## 範囲外のまま

- 値の集合、列・map、Alloyでの整数とrecord。
- `x.f` の更新の構文（今は`r - x -> x.r + x -> (値)`と書く）。
