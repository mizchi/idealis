# 0051: idealis-oracleによる差分テストと比較ベンチ

`crates/oracle`へ独立crate `idealis-oracle`を追加した。Rustの`#[test]`からgroup・case・reference・test_functionを登録し、check・bench・保存oracleのreplayで共有する。固定入力の比較を担当し、生成・縮小は既存のidealis-testingへ残した。[利用API](../../crates/oracle/README.md)と[plan](../plans/0051-oracle-library.md)を参照。

## 3実装のcold比較

同じTLA+ソース・設定を、TLC2 2.19 revision 5a47802、tla-ts 0.0.10、idealis nativeへ渡した。TLCはworker 1、seed 16、Java 21.0.13。Node 24.22.2、Rust 1.98.0のrelease buildを使った。各モデル7 sample・1 warmup・1 iterationで、下表は1呼び出しの中央値。sourceからの解析、process起動、輸送、出力解析、raw log書き込みを含む。

| モデル        | idealis native (ms) | tla-ts (ms) | TLC (ms) |
| ------------- | ------------------: | ----------: | -------: |
| linear-ring   |               4.505 |      71.607 |  584.650 |
| function-cube |               2.675 |      78.875 |  593.217 |

ringは1,000 distinct / 1,001 generated、8 bitのfunction-cubeは256 distinct / 2,049 generatedだった。TLCをreferenceとし、3実装ともcomplete・distinct・generatedの射影で一致した。冷起動の小さいモデルなので、JVM・Node起動の比重が大きい。これをTLC探索kernelに対する速度比や、大きいモデルでの優位性と解釈しない。

[正常照合](../runs/0051-oracle-02/check.json)、[raw sample・順序・範囲と比率](../runs/0051-oracle-02/cold-bench.json)を保存した。idealisのsample範囲はring 2.131–5.361 ms、function-cube 2.544–6.240 msと幅がある。信頼区間や有意差は計算していない。

## prepared Rustの費用

同じモデルを解析・コンパイルした後、通常のcheckとcheck_with_coverageを測った。20 sample・3 warmup・10 iterationsで、下表はsample合計をiterationsで割った時間の中央値。検査とserde JSONへの射影を含み、解析・コンパイル、process起動、照合と出力破棄は含まない。両経路の射影は保存TLC観測へ一致した。

| モデル        | check (µs) | coverage (µs) | medianの増分 |
| ------------- | ---------: | ------------: | -----------: |
| linear-ring   |    138.519 |       141.910 |       +2.45% |
| function-cube |    387.056 |       395.742 |       +2.24% |

[raw sample](../runs/0051-oracle-02/prepared-bench.json)では、予定818回のreference・preflight・warmup・測定が完了した。coldも予定56回を完了している。この測定はcoverageの改善前後を比較する実験ではなく、同じgroupで関数の費用を比較する利用例。約2%のmedian差から普遍的な追加費用を決めない。

## 差分検出と保存再生

countだけを壊すadapter、unknownを返すadapter、実行エラーを返すadapterを、同じ保存TLC観測へ照合した。[対照report](../runs/0051-oracle-02/controls.json)はそれぞれmismatch・unknown・errorを保持する。集約はerrorになる。人工故障であり、未知の実装バグ発見には数えない。

[drift対照](../runs/0051-oracle-02/drift.json)はpreflightとwarmupを通過し、最初の測定で値を変える。mismatchと失敗箇所を保存し、安定した実装を含め速度比をすべて無効化した。この対照の時計は10 ns刻みの論理clockであり、実性能のsampleではない。

[保存reportの再生](../runs/0051-oracle-02/replay.json)は6回の対象実装呼び出しだけで一致した。reference callbackは呼ばず、保存入力とTLC観測を使う。TLC自体は対象実装の1つでもあるため、targetとしては2回実行した。ホストはソース・バイナリ・lockfileのfingerprintと保存JSONのdigestを検査し、medianと速度比も再計算した。

## 最初の試行と計測の限界

[01の失敗ログ](../runs/0051-oracle-01/failure.log)を残した。cold/preparedの正常測定は一致したが、人工driftの小さい関数が0 nsになりunknownで停止した。0 nsを有効な高速sampleに変える修正はしていない。planへ追記し、人工driftだけを論理clockへ変えて02を新規実行した。01と02のsampleは混ぜていない。

今回の差分は有限モデルの判定と2つのcountに限る。到達state集合・反例trace・活性検査全体の互換性を、この射影の一致から主張しない。オラクルの独立性と観測の正規化、SUTの初期化はadapterが担当する。測定中はこの作業のbuild・test・他のTLC・profileを並走させなかった。

## 回帰・配布と採用

14本のRust挙動テストで、比較status、空suite、重複ID、保存判定・入力の不整合、preflight停止、warmup失敗、途中drift、呼び出し予算、clock逆行・0時間、統計と速度比を検査した。初回はAPI未実装によるcompile failureを確認してから実装した。独立crateとeval adapterのfmt/clippy/test、native/Wasm build、実Chromiumでの差分・保存再生・host clock経路が通った。Wasmにはhost importもcross-origin isolationも要らず、wall timeのブラウザ性能は測っていない。

just check/browser、runtime-build/test、evaluate/replay-evaluationを通した。8 crateを含むpackageと、idealis-oracleの単独consumerも通った。既存mutationは109件の保存結果を再利用し、104 killed / 5 survived。新crateのRust mutation検出力を測定した値ではない。チェックリスト検査のcrate数を8へ更新し、8本のoffline gateテストを確認した。Jevは全34比較で矛盾・未評価0件だった。

固定caseの実装間照合と正しさを確認する比較ベンチの共通基盤として採用する。自動PBT・縮小、Criterionの統計、並列測定、外部processの管理をcrateへ組み込む判断はしていない。次は実プロジェクトのSUTとモデルに接続し、countより強い観測・反例の診断費用と、既存手法に対する追加検出を評価する。
