# 0081: ralloyの引数付きpred/fun

[plan](../plans/0081-ralloy-parameterized-predicates.md)のとおり、引数付きの`pred`と`fun`、引数のない`fun`、`P[a, b]`・`a.P[b]`・`p.f`の呼び出し、box join（`r[x]`）、引数付きpredの`run`を加えた。group `predicates`は5モデル・23 commandすべてでAlloyと一致し、速度も`adopt`だった。

## 実装

- `Expr::Call`を`Expr::Apply(name, arguments)`に一般化した。parseの後に名前を解決する。
  - pred/funの呼び出しは`Apply`のまま残す。
  - それ以外の名前の`e[a, b]`は、box joinの`b.(a.e)`にする。
  - `a.P[b]`は`P[a, b]`に、引数のあるpred/funへの`p.f`は`f[p]`にする。
  - `run P`で`P`に引数があれば、`some 引数 | P[引数]`にする（`disj`の引数は`some disj`）。
- 呼び出しでは、引数を呼び出し側の環境で評価し、本体を引数だけの環境で評価する。引数の多重度は呼び出し時に検査しない。`out[out[x]]`のように集合を渡す呼び出しは、Alloyでも受け付けられて解けた（`nested-calls`の`TwoStep`）。
- 型検査は、pred/funの結果の型を覚えておき、呼び出しでは引数の数と項数を検査する。引数のあるpredを引数なしで参照するとtype error。
- 引数の多重度は既定の`one`だけ。`set`・`lone`・`some`は未対応として拒否する。

## 正しさ

| group        | モデル / command | 一致 | 残り                          |
| ------------ | ---------------- | ---- | ----------------------------- |
| `core`       | 13 / 50          | 50   | なし                          |
| `nary`       | 6 / 30           | 29   | 予算切れ1（0079から変わらず） |
| `hierarchy`  | 8 / 40           | 40   | なし                          |
| `predicates` | 5 / 23           | 23   | なし                          |

人工故障4件（引数を逆順に束縛する、本体に呼び出し側の局所変数を見せる、引数付きの`run`を`all`で量化する、`disj`の引数を無視する）をすべて検出した。Rust・TS・Chromium・`just check`・`wasm-test`・`package`・`evaluate`・`replay-evaluation`が通った。`semantics`とevalの対照から、未対応として拒否する例の`pred P[x: A]`を外した。

## 速度

[比較](../runs/0081-ralloy-comparison/result.json)。baselineは0080aのcandidate。13ケースすべてが両組で1.1以下で`adopt`。開発組は0.937〜1.074、確認組は0.991〜1.058。多くのケースで1を少し超えていて、0079から続く小さな固定費の増加の傾向は消えていない。負荷平均は約4〜5で、専用ホストではない。
