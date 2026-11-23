# 0078: ralloyとAlloyの差分テスト

ralloyをAlloyと互換にしていくための基盤を入れた。公式Alloy 6.2.0の答えをroracleの`CheckReport`として保存し、`cargo test`で再生する。今の対応範囲だけで書いたgroup `core`は、13モデル・50 commandすべてでAlloyと一致した。[plan](../plans/0078-ralloy-alloy-oracle.md)は、後続の0079〜0083の順序も決めている。

## 仕組み

- モデル: `crates/ralloy/tests/alloy/core/*.als`。commandにはすべてラベルを付ける。
- 観測値: commandごとに解があるか（runならinstance、checkならcounterexample）。
- 記録: `examples/record_alloy.rs`が各モデルをAlloy 6.2.0（`exec --solver sat4j`）で1回実行し、roracleの`check`でralloyと照合した`CheckReport`を`tests/alloy/core.json`へ保存する。jarのSHA-256、Javaの版、モデルのSHA-256は`core.provenance.json`。同じ入力で記録し直すと、byte単位で同じreportになった。
- 再生: `tests/alloy_oracle.rs`が現在のモデルを登録し、`replay`でralloyだけを実行する。予算切れ（`unknown`）、解析エラー、自分の`check_instance`で再現できないinstanceは一致にしない。runのinstanceは`matched`、checkの反例は`mismatch`（理由`property`）が期待値。

最初の記録では、checkの反例4件が`invalid`になった。原因はテスト側の誤りで、checkの反例も`matched`を期待していた。READMEどおり、checkの反例は`check_instance`で`mismatch`になるのが正しい。期待値をcommandの種類で分けて、記録し直した。

## 結果

| 項目                          | 値      |
| ----------------------------- | ------- |
| モデル / command              | 13 / 50 |
| 解あり / 解なし               | 37 / 13 |
| Alloyとの一致                 | 50 / 50 |
| 記録の時間（JVMの起動を含む） | 約13秒  |
| 再生の時間（`cargo test`）    | 約0.9秒 |

## 人工故障

評価器の6件を、再生テストで検査した。最初の12モデルでは、`one e`を`count >= 1`にする故障と、`lone e`を`count == 0`にする故障の2件が検出されなかった。どちらの式でも答えが分かれるcommandがなかったため。この2件を見てから、`cardinality-formulas.als`（6 command）を加えて記録し直した。結果を見てcorpusを足したことは、この記録に残す。加えた後は6件すべてを検出した。

| 故障                         | 最初の12モデル | 13モデル |
| ---------------------------- | -------------- | -------- |
| `one`を1以上として数える     | 検出されず     | 検出     |
| `lone`を0として数える        | 検出されず     | 検出     |
| 推移閉包を計算しない         | 検出           | 検出     |
| 反射閉包でidentityを足さない | 検出           | 検出     |
| 転置を恒等にする             | 検出           | 検出     |
| 含意の前件が偽で偽を返す     | 検出           | 検出     |

## 速度のbaseline

`benches/oracle.rs`は、各モデルの全commandの解析時間をCriterionで測る。探索で1回測ったところ、ほとんどのモデルは3 µs〜1 msで、`set-algebra`だけが約120 ms だった（scope 3で、setのfieldが2つあり、候補が約26万個になる）。この値は探索用で、採否には使っていない。0079以降は、各planで閾値を決めてから、旧版と交互の順で測る。

## 限界と次

- instanceがAlloyでも成り立つかは検査していない。検査しているのは、ralloy自身の評価器による`check_instance`だけ。
- Alloyの答えは、記録した時のjarとJavaに依存する。再生にはJVMは不要。
- 次は0079（関係の表現をn項へ一般化）。

```sh
just ralloy-oracle-record core
just ralloy-test
just ralloy-oracle-bench
```
