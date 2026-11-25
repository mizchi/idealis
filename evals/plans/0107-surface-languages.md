# 0107: モデルの書き方の比較（Quint・ML・Isabelle・Idris・Pkl）

これまでQuintを参考にしてきた。mizchiの問い: 関数型言語、とくにML系に寄せた方が自然に書けるか。IsabelleやIdrisにモデルのDSLを埋め込む、逆にPklで限られた範囲を書く、を使いやすさで比べ、表現で詰まないかを確かめる。採用の判断ではなく、比較の材料を作る。

## 題材（結果を見る前に固定）

意味はidealisの核（参照評価器）の意味で、`fixtures/rmodel`のモデルと同じ判定になるものを書く。

1. **permissions**: `fixtures/rmodel/permissions.model`。関係・合成（join）・量化・設定のfact。`NoLeak`・`OnlyGranted`が成り立ち、`SecretRead`に到達でき、`UnclearedSecret`に到達できない。
2. **bank**: `fixtures/rcheck/bank.model`。整数の属性・算術・guard。`NonNegative`が成り立ち、`Rich`（残高3以上）に深さ1で到達できる。
3. **order**: 代数的なデータ型。注文の状態が`Draft | Placed | Shipped(tracking: Carrier) | Cancelled(reason: Reason)`、`ship`は`Placed`だけを`Shipped`へ、`cancel`は`Draft`か`Placed`を`Cancelled`へ移す。性質: 「`Shipped`の注文は取り消されない」（`cancel`後に`Shipped`へ戻らない）、「`Fraud`で取り消された注文に到達できる」。中身の取り出しは`match`かそれに相当するもの。
4. **sessions**: `fixtures/rmodel/sessions.model`。数え上げ（`#active < 2`）。`Capacity`が成り立ち、`ThreeActive`に到達できない。
5. **service**: 活性とfairness。要求の集合と処理する`serve`。「`serve`に弱いfairnessがあれば、出した要求はいつか処理される」が成り立つ。

## 書き方（7つ）

1. rmodel（今の核の構文）: `rmodel::parse`と参照評価器で検査する。
2. Quint（本物）: `quint typecheck`と、できれば`quint verify`/`run`。
3. OCaml: 小さなDSLのmoduleへの埋め込み。`ocaml`で型検査する。
4. Isabelle/HOL: datatype・record・遷移関係の定義と、性質の命題（証明は求めない、`sorry`可）。`isabelle build`で検査する。
5. Idris2: 依存型での埋め込み。型検査を通す（証明は穴でよい）。
6. Pkl: 限られた範囲の記述（データとしてのモデル）。`pkl eval`で検査する。
7. Veil（Lean 4、参考）: [Veilの試し](../experiments/veil/README.md)と同じ環境。

## 測ること

- **表現**: 題材ごとの性質と構造を、直接書ける・回避策が要る・書けない、のどれかに分ける。回避策は内容を記録する。
- **検査**: その言語のtoolchainで検査が通るか。通らないものは合格に数えない。
- **大きさ**: 行数と、非空白の字句の数。
- **核への写し**: 核のIRへ写す手間（構文の解析が要る・ホストの値から読める・写せない）。

読みやすさ（0098のループ、新しい読み手）は、書き終えてから別に測る。書き手は同じ（Claude）なので、書きやすさの比較には書き手の偏りが入る。

## 範囲外

- どれかを採用すること、核の意味論の変更。
