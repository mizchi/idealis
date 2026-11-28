# 0108: 核のIRのJSONと、任意のadapter、roracleでの差分テスト

[plan](../plans/0108-ir-adapters.md)のとおり、rmodelの型検査前の構文木をJSONにした**核のIR（version 1）**を契約にし、表の構文からIRを書く**adapter**と、adapterの判定を**roracle**でrmodelの文の判定と照らす差分テストを作った。

```
rmodelの文 ─┐
Pklの木    ─┼→ 核のIR（JSON、version 1）→ idealis_model::validate（型検査）→ 参照評価器・rtlc・SAT・型の生成・照合
（任意）    ─┘
```

- IR: `crates/idealis-check/src/ir.rs`。式は`name`・`none`・`union`・`difference`・`intersection`・`product`・`join`・`literal`・`count`・`add`・`sub`・`mul`・`record`、論理式は`in`・`not_in`・`eq`・`ne`・`lt`〜`int_ne`・`some`・`one`・`lone`・`no`・`not`・`and`・`or`・`implies`・`all`・`exists`。adapterは型を決めない（型検査はrmodel）。写せなかった部分は`omitted`。
- adapter: rmodelの文（`idealis-check generate <model> ir`）と、0107のPklの木から書くPklのmodule（[ToIR.pkl](../experiments/surface-languages/pkl/ToIR.pkl)）。Pklの要素単位の木（`App`・`Image`・`In`・点ごとの更新）を、IRの関係の木（join・和・差・直積）に写す。payload付きの直和型と`match`を含むモデルは`unsupported`、stepとtemporalの性質は`omitted`。
- 差分テスト: `crates/idealis-check/tests/adapters.rs`（roracle）。入力は0107の題材と観測するcheckの名前、基準はrmodelの文、対象はPklのadapterの記録した出力。`just idealis-check-adapters`が出力を作り直して記録と比べ、テストを流す（CIに追加）。

## 正しさ

| plan の条件                                                                          | 結果                                                 |
| ------------------------------------------------------------------------------------ | ---------------------------------------------------- |
| IRの往復（corpus 9モデル、生成 seed 9800〜9899）                                     | IRが同じJSONに戻り、参照評価器の判定が変わらない     |
| Pklのadapterの判定がrmodelと一致（permissions・bank・sessions・serviceの`Disjoint`） | 4題材で一致（深さを含む）                            |
| orderとserviceの活性は一致に数えない                                                 | orderは`unsupported`でunknown、活性は`omitted`       |
| 人工故障2件の検出                                                                    | 下記                                                 |
| 知らない`op`・欠けたfieldを型検査の前に拒む                                          | 拒む（知らないfield・versionの違い・知らないsortも） |

人工故障（Pklのadapterの切り替え、`ADAPTER_FAULT`）:

- `Insert`と`Remove`の写しの入れ替え: permissionsとsessionsでmismatch。
- `Image`のjoinの向きを逆にする: permissionsとbankで**error**。planは「mismatchとして検出」としていたが、逆向きのjoinはsortが合わず、意味を比べる前にrmodelの型検査がIRを拒んだ。合格には数えない形で検出した。

## 範囲外のまま

- IRへの直和型・`match`・二状態の性質・時相の性質の追加（いまはadapterが`unsupported`・`omitted`にする）。
- Quint・ML風の構文のadapter、checker間の差分テストのroracleへの移行。
- 観測は判定と深さで、判定が偶然そろう書き換えの誤りは見逃しうる（人工故障の2件は検出した）。
