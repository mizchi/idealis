# roracleの評価

`crates/roracle`の同じ実装登録で、TLC・tla-ts・idealisの差分テストと比較ベンチを動かす。条件は[0051のplan](../../plans/0051-oracle-library.md)で固定した。

```sh
just oracle-test
just portable-build
just oracle-eval evals/runs/0051-oracle-<新しいID> <java> <tla2tools.jar>
just oracle-replay evals/runs/0051-oracle-<同じID>
```

TLCはrevision `5a47802`、worker 1、seed 16。runは新しいディレクトリだけに作る。測定と並行してbuild・test・他のTLCを実行しない。再生用ログも上書きしないため、同じrunの実装再生は1回だけ行う。

`linear-ring`は1,000状態、`function-cube`は8 bitの256状態。cold比較は7 sample・1 warmup・1 iterationで、プロセス起動・輸送・出力解析・raw log書き込みまでを含む。prepared比較は同じモデルのRust通常検査とcoverage付き検査を20 sample・3 warmup・10 iterationsで測り、検査とJSONへの射影を含める。両方とも観測はcomplete・distinct states・generated statesへ射影する。coverageの中身や反例traceの同値までは今回の射影で検査しない。

`manifest.json`は実行バイナリ・ソース・lockfileのhashを保存する。`check.json`の入力とTLC観測を使い、replayではTLCをreferenceとして再実行しない。TLCを測定対象として登録したcallbackは再生時も実行する。保存sampleのmedianと比率も再計算する。壊したcount・unknown・error・測定中のdriftは人工故障の対照で、未知の実装バグ発見には数えない。

ブラウザテストはimport不要のWasmで差分・保存再生・論理clockを実行する。ブラウザのwall timeの性能測定は行わない。
