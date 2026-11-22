# 0074への追記: cache keyの遅延計算と再測定

[0074の比較](../runs/0074-smt-comparison/result.json)は`hold`になった。`script-parse-ite-64`のcandidate/baseline比が開発3.522、確認3.709で、後退の閾値1.1を両組で超えた。逆順の確認でも同じ大きさなので、測定のばらつきとは扱わない。`script-parse-repeat-64`と`script-parse-scopes-32`は閾値を満たしたが、増分解析だけを測った探索値より遅かった。

原因の仮説は、`Script::parse`がcacheを使わない場合も各checkでsourceのSHA-256をfinalizeし、keyのSHA-256も作っていたこと。ite-64はassertが少なくcheckが64回あるため、この固定費が支配的になる。keyを`solve_incremental`で必要になったときだけ作る変更を探索的に測り、`script-parse-ite-64`が21.94 µs（0074のbaselineは22.11 µs）に戻ることを確認した。この探索値は採否に使わない。

変更はkeyの計算時期だけ。queryはhash状態とassumptionsを保持し、keyの値・cacheの意味・SolverのAPIは変えない。

再測定は1組だけ行う。0074のbaseline bench binary（`.artifacts/smt-incremental/baseline/solver-bench`）を再利用し、新しいcandidateのsolver/cache benchをビルドし直す。baseline→candidateの開発、candidate→baselineの確認、cache benchの開発・確認を、新しいrunディレクトリ（`0074a-*`）へ保存する。Criterionの条件、18/15ケース、閾値は0074のplanと同じ。0074のrunは上書きせず、`hold`の記録として残す。

この再測定でも閾値を超えたケースは、後退として記録し、さらに再測定しない。正しさのrun（seed 0x74a1/0x74c3）はこの変更後のcandidateで実行する。
