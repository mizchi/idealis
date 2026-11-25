# 0106: 帰納法の検査器の試作（proto）

[Veilの意味論の抽出](../experiments/veil/README.md)で、Veilの帰納法の自動化の本体はSMTだと分かった（wpからVCを作り、一階論理に落とし、cvc5に渡す。unsatを既定で信頼する）。mizchiの判断で、同じことをidealisの核のIRから直接行う検査器を、**試作として**作る。crateには入れず、採用は別に判断する。

## 試作の中身（`evals/experiments/induction/`）

- 対象の断片: sig・enum、`rel`・`var`の単項と二項と多重度、和・差・積・直積・join・`none`、比較、多重度の論理式、量化、`not`・`and`・`or`・`implies`。`#`・整数の値・record・和のsortは対応しないと答える。
- VC: invariantごとに、初期化のVC（fact・多重度・`init`から、そのinvariantが成り立つ）と、actionごとのVC（fact・多重度・すべてのinvariantを前の状態で仮定し、guardと代入（暗黙のframeを含む）と、後の状態の多重度から、そのinvariantが後の状態で成り立つ）。`Declarations`は、後の状態の多重度を示すVC。sigは解釈しないsort（すべての大きさ）、関係はBoolの関数、式は「tupleが式に属する」一階の式に写す。
- solverはz3の実行ファイル。unsatは帰納的、satは帰納的でない（CTI）、それ以外は未確定。
- CTIの検査し直し: satのとき、sortの大きさを小さい順に固定して具体的なCTIを取り出し、そのCTIを、sigをenumにした「初期状態がCTIの前の状態、actionがCTIのaction instanceだけ」のモデルに書き直して、参照評価器にかける。初期化のCTIは深さ0、actionのCTIは深さ1で、そのinvariantが破れること。

## 正しさ（結果を見る前に固定）

- corpus（`fixtures/rmodel`・`spec/models`）の、断片に入る全invariantと`Declarations`で:
  - 帰納的と答えたものは、参照評価器が破れると答えない（scopeの中で）。
  - 参照評価器が破れると答えたものを、帰納的と答えない。
  - 帰納的でないと答えたもののCTIは、すべて検査し直しで確かめられる。
- 生成したモデル（seed 9100〜9399、断片に入るもの）でも同じ3つ。
- 人工故障: VCで後の状態の多重度の仮定を外すと、`Declarations`の違反（参照評価器）とのずれで検出されるか。代入の暗黙のframeを外すと、CTIの検査し直しで検出されるか。

## 範囲外

- unsatの証明書、cvc5、CTIの前の状態への到達可能性（scopeの中）、時相の性質、`reachable`の問い、rcheckへの登録。
