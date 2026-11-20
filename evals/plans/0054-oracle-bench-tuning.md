# 0054: oracleとworkspaceベンチの通信・照合費用を改善する

対象は0051/0052で追加したidealis-oracleとworkspace比較で、TLA/SMTの探索アルゴリズムは変えない。既存cost測定は親のserde_json::to_writerがunbuffered ChildStdinへ細かく書き込む費用と、JS snapshot/Wasm/reportの費用を含む。小さいsample数とwarmupも見直す。

## 固定する比較

変更前のrunner・JS adapter・Wasm・native oracleをsource/hashとともに.artifactsへ保存する。実rtla/core応答の2入力を固定し、同じNode binary・payload・判定・親runner条件で比較する。3 stageは旧transport＋旧bridge、buffered transport＋旧bridge、同じbuffered transport＋新bridge。各stageでlegacy Node比較を対照にする。stage順をbefore/transport/bridge/bridge/transport/beforeとし、20 samples・3 warmup・25 iterationsを使う。入力・raw ns・実行順・hashを保存する。未知・不正・失敗、途中driftとprocess終了後の失敗で比率を出さない既存契約を維持する。

profileは通常測定と分け、samplyまたはNode CPU profileを使う。まず細かいpipe writeが費用を占めるか確認する。通信のserializationは引き続き時計内に置き、BufWriterでwriteをまとめる。payloadを時計外で事前serializeして高速化と数えない。

## 照合器の変更候補

固定入力のCriterionを追加する。Value出力とJSON bytes出力、単独・32 case・入れ子・不一致/unknownの正常対照を測る。JSON bytes APIは元のdispatch＋serde serializationと同じ契約で先に実装し、最適化前後で同じbenchを使う。通常測定は20 samples、warmup 0.5 s、measurement 1 s。before/after/after/beforeの順に保存したbenchmark executableを動かす。

Rustのwire境界はOracleGroupの比較を保ち、観測JSONのdeep cloneを借用に置き換える。bytes出力は中間serde_json::Valueを組み立てずにserializeし、Wasmは出力bufferを再利用する。JSはTextEncoder/Decoderの再利用に限り、snapshotの表現・検証とfull reportを保つ。hashだけの一致、成功だけのreport、省略した比較、結果cacheは使わない。defaultのJSON report schemaとstatusは変えない。

目標はrtlaのroundtripでtransport改善1.5倍以上、wire bytesのCriterionで1.2倍以上。coreとfailure/unknownの対照は10%以上の後退を採用判断へ明記する。目標未達やばらつきも残し、測定の有意差を自作しない。独立した旧native/Wasmのfull responseへ照合し、入力・拒否・不一致の判定を変えないことを優先する。

## 継続して使う測定条件

costの既定値を20/3/25へ揃える。workspaceはcold 10 samples・2 warmup・1 iteration、reused 20/3/25として、実行回数上限を事前検査する。coldとreused、transportとchecker kernelを別指標にする。過去の7/1/3と混ぜて速度比を作らない。

TDDでbytes API、full report互換、失敗、large payloadのflushとEOF、mutable入力の再照合を検査する。既存native/Wasm/browserと移行した差分テスト、cross/async、check/browser、runtime/evaluate/replay、配布consumerを確認する。SUPPORT/JevとCI、justfileへ追加する。測定中は他のbuild/test/profileを動かさず、raw runを上書きしない。PR/push/mergeは行わない。

## 測定開始前のwarmup単位確認

BenchConfig.warmupはbatch数ではなく関数の呼出回数だった。通常測定を始める前に、reused/costのwarmupを3 sample相当の75呼出（3×25）へ固定する。20 samples・25 iterations、上限10000は維持する。初期samplyは3呼出のprofileとして保存し、性能値には使わない。
