# 固定instanceの評価

実装からのsig/fieldを固定して、scope・宣言・fact・propertyの可否を検査する。公開APIは[利用手順](../../../docs/alloy.md)、事前条件は[0035の計画](../../plans/0035-alloy-instance.md)を参照。

```sh
just alloy-test
just alloy-browser
just alloy-instance-eval development evals/runs/my-instance-development /path/to/java /path/to/alloy-6.2.0.jar
just alloy-instance-eval confirmation evals/runs/my-instance-confirmation /path/to/java /path/to/alloy-6.2.0.jar evals/runs/my-instance-development
just alloy-instance-replay evals/runs/my-instance-confirmation
```

公式Alloy 6.2.0/SAT4Jは参照比較だけに使う。JARとJava 21を別途用意する。各caseでsig/fieldを固定するfactを加え、Feasibleで宣言・factの充足可能性、Queryでpropertyの真偽を検査する。公式側の実行エラーをinvalidへ変換しない。

開発seedは0x35a1の16件、確認seedは0x35c3の32件。固定10件も両runで反復する。正解は独立したTSのグラフ走査から作り、native JSONLと公開Wasm APIを照合する。生成ケースは2-3 nodeで、自己loopをfactで除外する場合と含む場合、run/checkを扱う。拒否対照と0/1 workは契約テストでも検査する。

新しいrun IDへソースとbinary/JARのhash、全入力・全応答、公式receiptとログを保存する。replayは証拠hashを照合し、同じソース・同じnative/Wasmで応答を再現する。公式Analyzerはreplayで再実行しない。以前に観測したseedの反復は、新しい独立確認には数えない。
