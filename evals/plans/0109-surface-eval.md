# 0109: 表の構文の評価ループ（軸を差し替えられる形）

[0107](../iterations/0107-surface-languages.md)の書き比べと[0108](../iterations/0108-ir-adapters.md)のadapterを、一度きりの比較で終わらせず、評価の軸を差し替えながら回し続けられるループにする。mizchiの挙げた軸: DDDの記述言語としての良さ、形式化との相性、機械での理解しやすさ、人間への理解しやすさ。

## ループ（`evals/experiments/surface-eval/`）

- **軸**は、`axes/`のmoduleが1つずつ宣言する: id、問い、指標、測り方（`automatic`: 道具で測る / `agent`: 新しいsubagentに問う / `rubric`: 記録した証拠から数える）、得点の付け方。軸を足すにはmoduleを足し、registryに登録する。
- **候補**（書き方）と**題材**はregistry（`candidates.json`・`tasks.json`）。題材ごとに、人間の軸の問い（振る舞いについて、性質の判定ではない）と答えを結果の前に書く。
- **plan**（JSON）が、使う候補・題材・軸と、agentの軸の読み手の数・modelを選ぶ。
- **手順**: `prepare`（automaticとrubricの軸を測り、agentの軸の問いを書く）→ `collect`（筆者がsubagentを起こし、答えをrunへ保存する）→ `score`（軸ごとの成績表、重みはplanにあれば）。`score`は保存した答えから何度でもやり直せる。runは上書きしない。
- 成績表は軸ごとに出し、1つの点にまとめない（planが重みを持つときだけ参考の合計を付ける）。

## 最初の軸（v1）

1. **ddd**（rubric）: DDDの概念（同一性を持つentity、値オブジェクト、payload付きの直和、集約の状態、command、不変条件、多態のentity、設定と前提）を、0107の書き手の報告から直接1・回避0.5・書けない0で数える。
2. **formal**（automatic + rubric）: 核のIRへのadapterがあるか、IRへ写せた性質の割合と、roracleでrmodelと一致した割合（`just idealis-check-adapters`の観測）、0107で答えを出せた検査の種類（完全な探索・証明・SMT・無作為のsimulation）。
3. **machine**（automatic + rubric）: 解析できる木を残すか、toolchainの検査が今も通るか（各候補の検査commandを再実行）。
4. **human**（agent、代理）: コメントを除いた各候補の題材を、構文の説明なしで新しい読み手（haiku）に渡し、振る舞いの問いに答えさせた正答率。本物の人間の読み手ではない。
5. **cost**（automatic）: 空行とコメントを除いた行数、検査commandの所要時間。

## 正しさ（結果を見る前に固定）

- 軸を1つplanから外す・足すと、成績表のその列だけが変わること（registryの差し替えの検査）。
- `score`が保存した答えから同じ成績表を再現すること。
- humanの問いと答えは、このplanのcommitに含める（`tasks.json`）。読み手に渡す文には、コメントと期待の判定の注記を含めない（Pklの`expect`のように構造に入っているものは、残ったことを記録する）。

## 最初のrun

0107の7候補・5題材・5軸。humanは候補ごとに読み手1人（dev）。

## 範囲外

- 本物の人間の読み手、機械の書きやすさ（新しいagentに書かせる軸）は軸として宣言だけし、v1では測らない。
