# ralloyの参照比較

静的なAlloyサブセットを独立したRust/Wasmで検査する。対応構文・API・探索結果の意味は[crateのREADME](../../../crates/ralloy/README.md)、条件は[0034の計画](../../plans/0034-alloy-subset.md)を参照。

## ローカルの契約・ブラウザ検査

```sh
just ralloy-test
just ralloy-contract
just ralloy-browser
just ralloy-example
just package
```

contractはnativeとWasmの応答を照合する。Chromiumではホストimportとcross-origin isolationなしで反例・予算切れ・構文拒否を確認する。packageはralloy単独のarchiveを別consumerで実行する。

## 公式Analyzerと比較する

[公式Alloy 6.2.0の配布JAR](https://github.com/AlloyTools/org.alloytools.alloy/releases/tag/v6.2.0)とJava 21を用意する。JVMは参照比較だけに使い、crateとWasmはJVMを必要としない。

```sh
just ralloy-eval development evals/runs/my-alloy-development /path/to/java /path/to/org.alloytools.alloy.dist.jar
just ralloy-eval confirmation evals/runs/my-alloy-confirmation /path/to/java /path/to/org.alloytools.alloy.dist.jar evals/runs/my-alloy-development
just ralloy-replay evals/runs/my-alloy-confirmation
```

各runは新しいディレクトリを必要とする。開発seedは0x34a1で32件、確認seedは0x34c3で64件。固定ケースも毎回実行する。TSの集合演算・グラフ走査で期待値を独立に計算し、native/Wasm/公式Analyzerの判定を比較する。返却構造はsigとfieldを固定する別commandへ変換し、公式Analyzerが受け入れることも検査する。

同じモデルで`Feasible` commandも実行するため、checkの反例なしと、factを満たす構造がない結果を区別できる。未対応・型・scope違反とwork予算切れは成功へ変換しない。

manifestにsource/binary/JARのSHA256、参照版、seedと全入力を記録する。release assetに公式checksumはなく、JARのhashは取得したファイルの観測値。各caseのソース、公式receipt、stdout/stderrも保存する。replayは証拠hashを検査し、現在の同一ソース・同一native/Wasmで全応答を再現する。replay時に公式Analyzerは再実行しない。

同じ確認seedの再実行は回帰検査であり、新しい独立した確認実験には数えない。2-cycleを使う実装比較も、既知の人工故障の再現で、未知バグの発見実績ではない。速度比較や既存backendへの統合はこの評価に含めない。
