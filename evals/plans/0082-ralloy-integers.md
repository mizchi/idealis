# 0082: ralloyの整数とcardinality

[0078](0078-ralloy-alloy-oracle.md)の5段目。今のralloyは、Int atomを宇宙に含めるだけで、整数式を扱わない。

## 記録したAlloyの答えから分かったこと

group `integers`（5モデル・31 command）を記録した。Alloy 6.2.0のCLIの既定では、整数はbit幅の2の補数で桁あふれする（wraparound）。

- 3 bitで`plus[3, 1] = -4`（`Wrap`）、`minus[-4, 1] = 3`（`NegWrap`）が解ける。
- 3 bitで`#A = 4`（`Overflow`）が解け、5個のatomで`#A < 0`（`CountWrap`）も解ける。
- 4 bitで`#Int = 16`（`IntSize`）が解ける。16個のatomの数もリテラルの16も、ともに0になる。

`arithmetic.als`の`#A.plus[1]`は、Alloyが名前を解決できないとして拒否した（組み込みの`plus`はreceiverの形で呼べない）。そのcommandを外して記録し直した。答えを見て変えたのではなく、Alloyで成り立たないモデルを直したもの。

## 変更

- 整数式: 整数リテラル（`-2`を含む）、`#e`、`plus`・`minus`・`mul`・`div`・`rem`・`negate`の呼び出し。比較: `<`、`>`、`<=`、`=<`、`>=`。`=`と`!=`は、片方が整数式なら整数の比較。
- Int型の単項の関係（`a.balance`、`Int`に量化した変数）を整数として使う時は、含むInt atomの値の和に変換する。
- すべての整数の値（リテラル、`#`、算術、変換した和）を、commandのbit幅の2の補数で桁あふれさせる。`div`と`rem`は0へ向けて切り捨てる（Javaの`/`と`%`）。0での割り算は未対応のまま評価しない（corpusに含めていない）。
- 整数を関係として使うこと（`x.balance in 3`など）は対象外とし、type errorにする。

## 正しさ（結果を見る前に固定）

- group `integers`がAlloyの答えと全一致すること。
- `core`・`nary`・`hierarchy`・`predicates`の再生が0081と同じ状態のままであること（`nary`の1件だけが予算切れ）。
- 人工故障: 桁あふれさせずに計算する、Int型の関係を値の和ではなくatomの数で整数にする、`<=`を`<`にする、リテラルを桁あふれさせない、の4件を再生テストが検出すること。

## 速度（結果を見る前に固定）

0081のcandidateの保存binaryをbaselineとして、`core`の13ケースを交互の順に測る。13ケースすべてで、両組のcandidate/baselineの比が1.1以下。超えたケースは後退として記録し、閾値を変えない。
