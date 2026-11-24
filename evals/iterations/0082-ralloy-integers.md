# 0082: ralloyの整数とcardinality

[plan](../plans/0082-ralloy-integers.md)のとおり、整数リテラル、`#e`、`plus`・`minus`・`mul`・`div`・`rem`・`negate`、整数の比較を加えた。group `integers`は5モデル・31 commandすべてでAlloyと一致し、速度も`adopt`だった。

## Alloyの答えから決めた整数の意味

記録したAlloy 6.2.0の答えでは、CLIの既定で整数がbit幅の2の補数で桁あふれした。

- 3 bitで`plus[3, 1] = -4`と`minus[-4, 1] = 3`が解ける。
- 3 bitで`#A = 4`が解け（4も-4になる）、5個のatomで`#A < 0`も解ける。
- 4 bitで`#Int = 16`が解ける。atomの数もリテラルも、ともに0になる。

ralloyでも、リテラル・`#`・算術・関係から変換した和のすべてを、commandのbit幅で桁あふれさせる。`div`と`rem`は0へ向けて切り捨てる。0による割り算はcorpusに含めておらず、評価すると`unknown`にする。

`arithmetic.als`の`#A.plus[1]`は、Alloyが名前を解決できないとして拒否した。組み込みの`plus`はreceiverの形で呼べない。そのcommandを外して記録し直した。

## 実装

- tokenizerに`<`、`>`、`<=`、`>=`、`=<`、`#`と整数リテラル（`-2`を含む）を加えた。
- 型に`Integer`を加えた。Int型の単項の関係（`a.balance`、Intに量化した変数）は、含むInt atomの値の和として整数に変換する。`=`と`!=`は、片方が整数なら整数の比較。
- `plus`などの組み込みは、同名のpred/funがなければ呼び出しとして扱い、box joinにしない。
- Int atomは宇宙の先頭の2^bit幅個で、番号から値は`番号 - 2^(bit幅-1)`。

## 正しさ

| group        | モデル / command | 一致 | 残り                          |
| ------------ | ---------------- | ---- | ----------------------------- |
| `core`       | 13 / 50          | 50   | なし                          |
| `nary`       | 6 / 30           | 29   | 予算切れ1（0079から変わらず） |
| `hierarchy`  | 8 / 40           | 40   | なし                          |
| `predicates` | 5 / 23           | 23   | なし                          |
| `integers`   | 5 / 31           | 31   | なし                          |

人工故障4件（桁あふれさせずに計算する、Int型の関係を値の和でなくatomの数にする、`<=`を`<`にする、リテラルを桁あふれさせない）をすべて検出した。Rust・TS・Chromium・`just check`・`wasm-test`・`package`・`evaluate`・`replay-evaluation`が通った。`semantics`とevalの対照から、未対応として拒否する例の`#A = 1`を外した。

## 速度

[比較](../runs/0082-ralloy-comparison/result.json)。baselineは0081のcandidate。13ケースすべてが両組で1.1以下で`adopt`。開発組は0.938〜1.028、確認組は0.974〜1.079。負荷平均は約4〜5で、専用ホストではない。
