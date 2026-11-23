# 0074への追記: `prepared-128`の再測定

[0074bの比較](../runs/0074b-smt-comparison/result.json)は`hold`になった。`prepared-128`のcandidate/baseline比が開発組0.965、確認組1.235で、確認組だけが後退の閾値1.1を超えた。確認組のcandidate runは中央値の95%区間が202〜256 µsと広く、ほかの3 runの区間（幅1.5〜4 µs）と違う。0074bの変更は`Script`のkey保持だけで、`Problem::solve`の経路にはない。

[0074bのplan](0074b-smt-memoized-cache-keys.md)は、超えたケースを後退として記録し、さらに再測定しないと決めていた。この追記は、その決定を変える。mizchiの指示による。[0074のplan](0074-smt-incremental-cache.md)の「1組だけ再測定を追記planで宣言する」に戻し、0074bの記録は`hold`のまま残す。

## 固定する条件

- bench binaryは再ビルドしない。0074bと同じbaseline（`.artifacts/smt-incremental/baseline/solver-bench`）とcandidate（`.artifacts/smt-incremental/candidate-memo`）を、保存したprovenanceのhashで確認して使う。
- Criterionの条件、18ケース、閾値は0074のplanと同じ。`prepared-128`だけを選んで測らず、18ケースすべてを測る。
- 順序は0074bと同じ。baseline→candidate（開発）、candidate→baseline（確認）の4 runを`0074d-*`へ保存する。ほかの重い処理と同時に測らない。

## 判断

- 18ケースすべてが両組で閾値内なら`adopt`とし、0074bの`prepared-128`は再現しなかった後退として記録する。
- どれかが超えたら、そのケースを後退として記録する。この追記の後は再測定しない。ほかのケースが新しく超えた場合も同じ扱いにする。
- 0074bの結果と組み合わせて、都合のよい組を選ばない。判断はこの4 runだけで行う。

0074bのrunは上書きしない。cache benchとcorrectnessのrunは再実行しない。
