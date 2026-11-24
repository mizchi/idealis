# 0084への追記: 対称性の準備の固定費

[0084の比較](../runs/0084-ralloy-comparison/result.json)は`hold`になった。数µsの6ケースが1.10〜1.34倍になった（`core-sig-multiplicity` 1.343/1.293、`core-cardinality-formulas` 1.256/1.259など）。`core-set-algebra`は0.910/0.944、`core-dense-graph`は0.884/0.930だった。

小さいモデルでは候補が少なく、worldごとの準備の費用が見える。準備では次のものを毎回作っている。

- slotの位置を引く`BTreeMap`
- 互換ごとの対応表`Vec<usize>`
- 2列以上のfieldのtupleの置換

変更は2つ。

- slotの位置を、fieldとatomの番号で引く固定長の表（`Vec`）で求める。
- 入れ替えられる組がないworldでは、何も作らない。

探索の順序・刈る枝・結果は変えない。

測定は0084と同じ手順で1組だけ行う。baselineは0084と同じ保存binary（0083aのcandidate）、candidateはこの変更のcommit。閾値は0084と同じ（13ケースすべてで1.1以下）。結果は`0084a-*`へ保存し、0084のrunとは組み合わせない。超えたケースは後退として記録し、さらに再測定しない。mizchiの指示で、小さいcaseの固定費は目立つ問題にならない限り扱わないため、これより先は追わない。
