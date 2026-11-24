# 0090: ralloyの時相をSATで有界に検査する

[plan](../plans/0090-ralloy-temporal-bmc.md)のとおり、ralloyにAlloy 6のfieldの`var`と、`always`・`eventually`・`after`・primeを加えた。trace（状態の列）を長さstepsのlassoとしてSATへ展開して解く。静的な構造（sig・静的なfield）と、状態とともに変わるfieldを1つのモデルで扱える。

- **Alloyとの一致**: 時相の5モデル・25 commandすべてで、解の有無がAlloy 6.2.0と一致した。既存の5 group・178 commandも一致したまま。
- **witness**: SATが出したtraceは、すべて評価器の`check_instance`で検査し直して通った。runは`Matched`、checkは`Mismatch`（Property）。
- **時間**: 25 commandは、各0.04〜1.13 ms、合計7.08 ms（release、7回の中央値、負荷平均15.7の中）。採用の判定には使わない。
- **Wasm**: 578,403 bytesから603,772 bytesに増えた（rustc 1.100）。

## 意味論

- 長さk以下のlassoは、loopを1状態ずつ展開すると、長さちょうどkのlassoにできる（無限の語は変わらない）。このため、ちょうどkのtraceだけを探し、最後の状態の次をloopの変数で選ぶ。
- 時相の演算子のないfact・command本体は最初の状態で評価し、fieldの型と多重度は全状態で成り立たせる。静的なfieldは全状態で同じgateを共有する。
- 位置iから到達する状態は、i以降と、loopの戻り先以降。`always`・`eventually`は、iより前の状態を「loopがその状態か、それより前へ戻る」ときだけ数える。
- 時相モデルは総当たりで解かない。`div`・`rem`を含む時相モデルはエラーにする。

## 人工故障

| 故障                                                         | 検出                                       |
| ------------------------------------------------------------ | ------------------------------------------ |
| 最後の状態からloopへ戻る遷移を無視する（最後の状態で止まる） | Alloyとの差（`counter`の4 command）        |
| `always`・`eventually`でiより前の状態を数えない              | witnessの検査（`mutex/Released`がInvalid） |
| fieldの宣言を最初の状態だけにする                            | witnessの検査（`counter/Wraps`がInvalid）  |

3件とも検出した。2件は、Alloyの答えと比べる前に、SATのtraceを評価器が拒否した。

## 計画からの差

- planには29 commandと書いたが、結果を見る前に固定したcorpusは25 commandだった。planの数え違いで、corpusは変えていない。
- planの「SATの変数・節の数」は報告せず、workを報告した。変数の数を外へ出すAPIがないため。
- 0 stepsと33 steps以上はエラーにした（1〜32）。静的なモデルに書いたstepsは、Alloyと同じく受け付けて無視する。

[時間の記録](../runs/0090-ralloy-temporal/times.jsonl)は`cargo run --release --example temporal_times`で再現できる。

## 次の段階

- Quintのモデル（0089）を、このSATのBMCでも検査できるようにするには、Quintの値（整数・集合・map）を関係へ写す必要がある。0089のTLA+経路（明示状態）と、このSAT経路（記号的）を同じモデルで比べるのが次の候補。
