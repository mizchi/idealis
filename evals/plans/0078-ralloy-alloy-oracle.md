# 0078: ralloyのAlloy互換をroracleの差分テストで進める

mizchiの依頼は、ralloyをAlloyと互換にしながら、roracleで差分テストを入れ、速度も改善すること。対応を広げる機能は、sigの継承・subset・一括宣言、引数付きpred/fun、整数とcardinality、3項以上の関係の4つ。Alloyの答えは保存して再生する。速度は、今の全列挙を残したまま探索を改善する（SAT符号化への移行はしない）。

## 進め方

段階ごとに、Alloyのモデル群（group）を先に書き、Alloy 6.2.0の答えを記録してから実装する。

| plan | 内容                                                             |
| ---- | ---------------------------------------------------------------- |
| 0078 | 差分テストの基盤と、今の対応範囲のgroup `core`、速度のbaseline   |
| 0079 | 関係の表現をn項へ一般化し、3項以上のfieldに対応する              |
| 0080 | sigの継承・subset・abstract・一括宣言・signature fact            |
| 0081 | 引数付きpred/fun                                                 |
| 0082 | 整数とcardinality。bitwidthと桁あふれの扱いはAlloyの答えで決める |
| 0083 | profileに基づく探索の改善（対称性の除去、枝刈り）                |

関係の表現を先に変えるのは、ほかの3機能がその上に乗るため。

## 差分テスト

- モデルは`crates/ralloy/tests/alloy/<group>/*.als`。commandにはすべてラベルを付ける。
- 観測値は、commandごとに解があるかどうか（runならinstance、checkならcounterexample）。checkでfactが充足不能な場合を区別したい時は、別の`run {}`をモデルに書く。
- ralloyの`unknown`（予算切れ）、解析エラー、自分の`check_instance`で一致しないinstanceは、一致として扱わない。
- 記録: `examples/record_alloy.rs`が各モデルを`java -jar org.alloytools.alloy.dist.jar exec --solver sat4j`で1回実行し、roracleの`CheckReport`を`tests/alloy/<group>.json`へ保存する。jarのSHA-256、Javaの版、各モデルのSHA-256を`<group>.provenance.json`へ保存する。
- 再生: `tests/alloy_oracle.rs`が現在のモデルを登録して`replay`する。モデルを編集して記録し直していなければ、入力の不一致で失敗する。
- 限界: instanceがAlloyでも成り立つかは、記録時にも再生時にも検査しない。検査するのは、ralloy自身の評価器による`check_instance`だけ。0034のようにAlloyで固定して検査する方法は、0080以降で表現が広がってから検討する。

## 0078の範囲

- group `core`: 今の対応範囲だけで書いた12モデル、計50 command前後。
- 記録した時点でralloyとAlloyが一致しなかったcommandは、互換性の欠陥として記録する。結果を見てからモデルを書き換えない。
- 速度のbaseline: `benches/oracle.rs`で、group `core`の各モデルの全commandの解析時間をCriterionで測る（20 samples、warmup 1秒、measurement 2秒、median、95%区間）。0079以降の速度の比較に使う。採否の閾値は、各planで測定前に決める。
