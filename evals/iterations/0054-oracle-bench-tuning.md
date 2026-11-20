# 0054: oracleの通信とJSON照合を改善する

workspaceの比較runnerとidealis-oracleを最適化し、固定したrtla/core応答の往復照合をそれぞれ4.15倍、2.88倍に改善した。JSON bytes出力のCriterionは1.78〜2.26倍に改善した。これは収集済み応答の通信・照合費用で、TLA/SMTの探索速度やTLCとの速度比を表さない。

## 比較条件と集計

[事前計画](../plans/0054-oracle-bench-tuning.md)をコミットしてから変更した。Apple M5、Node v24.22.2で、旧runner・旧JS adapter・旧Wasm・旧nativeを保存した。source/binary hashは[集計](../runs/0054-summary/result.json)のsnapshotsへ残した。元の実行ファイルは`.artifacts/0054-{old,transport,after}`にあり、Gitへは含めていない。

[往復測定](../runs/0054-roundtrip/result.json)はbefore/transport/bridge/bridge/transport/beforeの順。20 samples・75 warmup呼出・25 iterationsで、全stageが2306/2306呼出を終え、全観測が一致した。bridgeはtransportと同じ親runnerを使う。各stageで独立したNode strict equalityを対照として動かした。

表は2 runそれぞれのmedianを取り、その2値のmedianをµs/呼出で示す。比率もこの代表値から計算した記述値であり、有意差や環境を越えた保証は主張しない。

| 固定応答の往復照合 |       旧版 | 通信のみ変更 | 通信＋bridge | 通信の倍率 | bridgeの追加倍率 | 全体の倍率 |
| ------------------ | ---------: | -----------: | -----------: | ---------: | ---------------: | ---------: |
| rtla               | 1747.52 µs |    553.65 µs |    420.83 µs |     3.16倍 |           1.32倍 |     4.15倍 |
| idealis-core       |   81.41 µs |     37.76 µs |     28.30 µs |     2.16倍 |           1.33倍 |     2.88倍 |

legacy Node対照もrtlaが1212.07→52.14→48.01 µs、coreが56.09→10.96→10.24 µsへ減った。大きな改善の一部は、比較対象へ共通にかかっていた親runnerの通信費用だった。変更後もoracleを使う照合はlegacyより遅く、rtlaで約8.77倍、coreで約2.76倍の費用がかかる。

## profileから変更した箇所

[変更前の親runner CPU profile](../runs/0054-profile-before/summary.json)は`write`のselfが996712/1265398、約78.8%だった。serde_jsonがChildStdinへ細かく書いていたため、cold/reusedの両経路をBufWriterでまとめた。返信を待つ前に明示flushし、serializationは時計内へ残した。[変更後](../runs/0054-profile-after/summary.json)は`read`が先頭となり、`write`は約29.1%になった。

profileは通常測定と分けた。変更前はwarmup 3呼出、変更後は計画の単位修正後の75呼出で、絶対CPU時間の速度比には使わない。初期Criterion測定も[補助記録](../runs/0054-criterion-before-1/stdout.log)として残し、表には次の固定順4 runだけを使う。

Rust wireはOracleGroupの比較を使い続け、観測とinputを借用する。新しい`dispatch_json`は全reportをbytesへ直接serializeする。Wasmは出力bufferを再利用し、JS adapterはTextEncoder/Decoderをinstanceごとに再利用する。JSON objectのkey順は変わり得るが、reportの内容とstatusは保持する。

## Criterionによる照合kernelの比較

[Criterion raw](../runs/0054-kernel/manifest.json)は同じbench実行ファイルのbefore/after/after/before。各20 samples、warmup 0.5秒、measurement 1秒。各runのmedian推定値を上と同じ方法で集計した。旧bytes経路は、変更前のdispatchとserde_json::to_vecを組み合わせたもの。parse・照合・全reportの構築を含み、入力の準備と出力の破棄は含まない。

| 入力          |       Value旧→新 | Valueの倍率 |       bytes旧→新 | bytesの倍率 |
| ------------- | ---------------: | ----------: | ---------------: | ----------: |
| 単独          |     4.04→3.35 µs |      1.21倍 |     4.63→2.60 µs |      1.78倍 |
| 32 cases      | 405.41→327.85 µs |      1.24倍 | 503.10→272.92 µs |      1.84倍 |
| 入れ子256要素 | 362.26→258.11 µs |      1.40倍 | 422.14→237.51 µs |      1.78倍 |
| unknown対照   |     7.62→5.73 µs |      1.33倍 |    10.25→4.53 µs |      2.26倍 |

事前目標のrtla通信1.5倍、bytes kernel1.2倍を満たした。coreとunknownの対照に10%以上の後退はなかった。32 casesのValue新版のrun medianは298.19/357.50 µsと振れたので、単一runの値で一般化しない。

## 互換性と失敗の記録

[56ケースの全応答比較](../runs/0054-compat-02/result.json)で旧native・旧Wasm・新native・新Wasmが一致した。7種の観測の全組み合わせ、不正JSON・空batch・version不正、大きな応答→不正→小さな応答の順を含む。JS入力の変更は次の照合へ反映し、過去reportを変えないことも回帰テストへ追加した。

[最初の互換検査](../runs/0054-compat/failure.json)は評価scriptの問題で中断した。resize前にmemory.bufferを読んだため、Wasmのmemory growthでArrayBufferがdetachされた。pointerを先に取得するよう直し、別runで全応答を再確認した。失敗記録は残した。

通常測定の開始時点ではこの補助scriptが失敗していたが、Rust/Nodeの契約テストと各runのpreflightは通っていた。その後の独立した全応答比較が一致したため、測定値を採用した。測定入力・製品コード・保存実行ファイルは変更していない。

## 継続する測定と確認

costの既定値は20 samples・75 warmup呼出・25 iterationsへ揃えた。workspaceのcoldは10/2/1、reusedは20/75/25とし、予定呼出上限10000を保つ。warmupはsample数ではなく呼出回数。旧7/1/3の記録とは別に比較する。[実行手順](../workspace-oracle/README.md)と`just oracle-criterion`を使う。

[検証記録](../runs/0054-validation/result.json)にcheck/browser、oracleのRust・JSONL・Wasm・Chromium、runtime、evaluate/replay、mutation、TS/Rust/Goのcross、async、配布consumer、SUPPORT/Jevを残す。mutationは既存cacheを再利用した95.4%であり、新しい変異の実行結果とは数えない。

通信bufferingと借用・直接serializationを採用する。次にWasm bridgeを詰める場合は、snapshot生成・Wasm内のparse・reportのdecodeをprofileで分ける。reportの省略、hashだけの一致、結果cacheは今回導入していない。
