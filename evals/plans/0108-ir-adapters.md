# 0108: 核のIRのJSONと、任意のadapter、roracleでの差分テスト

[0107](../iterations/0107-surface-languages.md)で、表の構文は系統ごとに得手不得手があり、解析できる木を残す書き方だけが検査器・型の生成・照合に使えると分かった。mizchiの方針: いったんrmodelに寄せ、表の構文は任意のadapterで書けるようにし、adapter間の差分はroracleで吸収する。

## 変更

- **IRのJSON（version 1）**: rmodelの構文木（型検査の前）をJSONにした形を契約にする。sort・enum・和・record・関係（形と多重度、`var`か）・fact・init・action（引数・guard・代入）・check・scope。式と論理式は`op`で区別する木。整数の算術は明示の`arith`。`rcheck ir <model>`がrmodelの文からIRを出し、`rcheck`はIRのJSONも入力に取る（型検査はrmodelの検査器で行う）。
- **adapter**: 「表の構文 → IRのJSON」だけを担う。式は木として渡す（ホストの関数を渡さない）。IRにない機能を含む部分は、`omitted`（名前と理由）として返す。最初のadapterは2つ: rmodelの文（rcheck自身）と、0107のPklの木からIRのJSONを書くPklのmodule（`pkl eval`）。
- **roracleでの差分テスト**: 入力は0107の題材。基準はrmodelの文 → IR → 参照評価器の判定（題材ごとに決めたcheckの名前の、判定と深さ）。対象は各adapterのIR → 参照評価器の判定。adapterが`omitted`にしたcheckは`unknown`として観測し、一致に数えない。

## 正しさ（結果を見る前に固定）

- IRの往復: corpus（`fixtures/rmodel`・`spec/models`・`fixtures/rcheck`）とseed 9800〜9899の生成モデルで、rmodelの文 → IR → 型検査 → IR が同じJSONになり、参照評価器の判定が変わらないこと。
- 差分テスト: 0107の題材のうちPklが書けるもの（permissions・bank・sessions・serviceの`Disjoint`）で、Pklのadapterの判定がrmodelと一致すること。orderは直和型のpayloadで`omitted`、serviceの活性も`omitted`と観測されること（一致に数えない）。
- 人工故障: Pklのadapterで`Insert`と`Remove`の写しを入れ替える、`Image`のjoinの向きを逆にする、の2件を、roracleの差分テストがmismatchとして検出すること。
- IRのJSONに知らない`op`・欠けたfieldがあれば、型検査の前に拒むこと。

## 範囲外

- IRへの直和型・`match`・二状態の性質・時相の性質の追加、Quint・ML風の構文のadapter、checker間の差分テストのroracleへの移行。
