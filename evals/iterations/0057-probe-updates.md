# 0057: 重複する関数更新のコピーを省く

汎用Scalar列を持つ関数・record・sequenceの更新で、値の列をコピーする前にintern tableを検索する。更新後の値が既にあれば、既存のIDを返す。新規の値だけを確保する。公開API、IR、JSON、探索対象は変えていない。

## 変更した検索

0056の関数表現はdomainを共有したが、更新のたびに汎用payloadをcloneしていた。重複した遷移でもcloneを行い、intern tableとの比較が終わると破棄していた。新しい`FunctionUpdate`は元の値と変更位置・変更値を借用し、更新後のhashと完全比較を行う。

検索ではdomainと全payloadを比較し、hashだけで同じ値とはしない。workerのbase/local両方を検索する。未登録の値は、検索に使ったhashを保持して値を確保する。深さとworkの検査は新規登録時の既存処理を通す。packed Bool/Intと、汎用列からpackedへ戻る更新は従来の経路を使う。

## 固定した条件

[plan](../plans/0057-probe-updates.md)は実装前にcommitした。基準は`5b72a9f9`で、0056の最終候補Dを含む。TLCの`FcnRcdValue.takeExcept`はローカルcloneの`d8fb231de4e4b5d02c1b2ed1ac759c086f694021`を読んだ。実行比較のTLC jarはmanifestへ別にhashを保存する。

大規模比較はfunction N20（1,048,576状態・20,971,521生成）とchannel N7（2,097,152状態・22,020,097生成）。tla-ts 1、TLC 1/8、before/after native 1/8、before/after Wasm 1を、1 warmup後3回測る。小さい4モデルの正常・違反版を全経路へ通し、72対照を検査する。全探索のcompleteと状態数・生成数を確認し、不完全な結果を速度比へ入れない。

主値は別processの起動、共通TS frontend、検査と応答を含むwall時間。Rust frontendだけのsource解析やprepared kernelの比較ではない。Node Wasmの結果はブラウザの速度ではない。Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、固定arm64 Java/TLCを使用する。heapは各8 GiBで、共通の厳密なRSS上限ではない。ホストの負荷変動を含み、全sampleと最小・最大を残す。

## 初期候補Aの後退

AはFunction全体を借用した。[Aの大規模比較](../runs/0057-comparison/result.json)では、channelの8 workersが5.92→5.55秒になった一方、functionの8 workersは2.35→2.50秒へ後退した。逐次とWasmの差は小さかった。BはdomainとScalar sliceを直接借用し、変更位置の前後をsliceで比較する。hashでも要素ごとのpayload表現判定を省き、packed guardにinline指定を付けた。Aのrunを編集せず、Bを同じ条件で別に測った。

## 大規模比較

Bの同じrunの秒数。中央値（最小-最大）を示す。

| 経路            |            0/1整数関数 |             チャネル群 |
| --------------- | ---------------------: | ---------------------: |
| tla-ts 1        |    5.648 (5.414-5.714) | 15.829 (15.732-15.894) |
| TLC 1           | 13.487 (13.463-13.727) | 23.260 (23.197-23.485) |
| TLC 8           |    4.391 (4.286-4.637) |    6.341 (6.111-6.674) |
| 変更前 native 1 |    3.870 (3.648-3.956) | 13.994 (13.735-14.042) |
| 変更後 native 1 |    3.910 (3.696-3.933) | 13.670 (13.670-13.747) |
| 変更前 native 8 |    2.548 (2.535-2.563) |    6.149 (5.632-6.480) |
| 変更後 native 8 |    2.609 (2.537-2.696) |    5.531 (5.416-5.699) |
| 変更前 Wasm 1   |    5.480 (5.304-5.636) | 20.734 (20.730-20.740) |
| 変更後 Wasm 1   |    5.595 (5.287-5.747) | 20.541 (20.535-20.703) |

channelはnative 1が1.02倍、native 8が1.11倍、Wasmが1.01倍。native 8はTLC 8比1.15倍だった。functionはnative 1で約1%、native 8で約2%、Wasmで約2%遅くなった。sampleの範囲は重なり、小さい差を統計的に有意とは主張しない。

channelのWasmはtla-tsより約30%遅い。functionのWasmは今回のbeforeもtla-tsより短く、0057で逆転したとは扱わない。0056との絶対時間の差にはホスト条件の変動が含まれる。

[Bのraw run](../runs/0057-comparison-b/result.json)と[集計](../runs/0057-summary-b/result.json)を参照。集計時に保存したdriver sourceとraw JSONのhashを検算した。

## Criterionとprofile

既存14本を同じbench sourceで測る。20 samples、warmup 0.5秒、measurement 1秒で、before/after/after/beforeの順。2回のmean推定値を算術平均し、pooled confidence intervalとして扱わない。小さいprepared fixtureで、frontend・IPC・起動を含まない。

単位はms。value/membershipは4096操作のbatch、searchは固定した小さいfixtureの全探索で、大規模比較と入力サイズは異なる。

| kernel                             | 変更前 | 変更後 | 前/後 |
| ---------------------------------- | -----: | -----: | ----: |
| `membership/function-cube`         |  1.551 |  1.504 |  1.03 |
| `membership/parallel-channels`     |  0.885 |  0.877 |  1.01 |
| `search/counter-product/1`         |  9.362 |  9.461 |  0.99 |
| `search/counter-product/8`         |  9.410 |  9.325 |  1.01 |
| `search/function-cube/1`           |  6.205 |  6.102 |  1.02 |
| `search/function-cube/8`           |  5.833 |  5.802 |  1.01 |
| `search/linear-ring/1`             |  1.079 |  1.079 |  1.00 |
| `search/linear-ring/8`             |  1.088 |  1.081 |  1.01 |
| `search/parallel-channels/1`       | 12.408 | 12.402 |  1.00 |
| `search/parallel-channels/8`       |  7.065 |  6.744 |  1.05 |
| `values/except-function`           |  0.443 |  0.442 |  1.00 |
| `values/except-record`             |  3.910 |  3.967 |  0.99 |
| `values/intern-existing-functions` |  0.375 |  0.363 |  1.03 |
| `values/intern-new-functions`      |  0.463 |  0.466 |  0.99 |

14本に10%以上の後退はなかった。channel探索の8 workersは1.05倍で、逐次はほぼ同じ。record更新単体は約1%遅かった。新規値が多い入力や、packedだけの入力に同じ利得を期待しない。

[Bのprofile](../runs/0057-profile-summary/result.json)はcounter M24、function N16、channel N5、ring 100,000をprepared IRから各16回、native 1/8で採取した。channel逐次のself比率はeval_operatorが9.1%、SipHasher::writeが8.1%、evalが6.5%、find_updatedが4.8%だった。式評価とhash生成の費用は残る。8 workersの待機sampleを並列効率の直接値にせず、profileのCPU sample構成を通常のwall速度比として使わない。既存0024のhelperを再利用し、実際のsource/binary hashを保存する。

## 互換性と検証

更新後の値を独立に構築して、borrowed viewのhashと完全比較を確認する。汎用列のままの更新、幅65、子のRefを含む値、packedへの復帰、base/localでのhash衝突をテストした。新しいview APIがない基準ではRedになり、実装後に通った。

Bの[旧版照合](../runs/0057-parity-b/result.json)は172入力・1,720コマンドで、before/after native/Wasmの全応答が一致した。幅0/64/65、型不一致、予算切れ・エラー・work・traceを含む。Rustの新テストでは幅1/2/7/64/65の全更新位置を確認し、packedへの復帰とbase/localでの衝突も検査する。

[大規模比較の24組](../runs/0057-verification/parity-proof.json)も、通常result全体が変更前後で一致した。71 source hashとnative/Wasm binaryは測定用snapshot Bへ一致した。Criterionの実行時fixtureは埋め込みfixtureとJSONとして同じだった。

[TLCの既存corpus](../runs/0057-tlc/result.json)は155件で、153一致・想定した拒否2件・不一致0。モデル評価エラー30件を合格へ置き換えず、105反例を検証した。[既存の確認用seed](../runs/0057-tlc-confirmation/result.json)も32件すべて一致した。[保存再生](../runs/0057-tlc-replay/result.json)も155件で判定・計数・反例が一致し、不一致0だった。

このseedは過去に使用したcorpusの回帰で、新しい独立した確認実験とは数えない。

`just check`、`browser`、`portable-test`、`safety-test`、`criterion-test`、eval contract tests、`runtime-build/test`、`evaluate/replay-evaluation`、`package`を実行した。Rustのrtlcはunit/integrationの50件とbench smoke、TSの共通78ケースはVitest/node:testで通る。Chromiumと単独のRust/TS/Wasm consumerも検査した。今回Rustのmutation scoreは測っていない。

SUPPORTは全8 crate・36比較のlive検査とオフライン再生を行い、矛盾・未判定0。[検証ログ](../runs/0057-verification/result.json)へRed、A/Bの測定、最終検証を分けて保存する。

## 再実行

```sh
IDEALIS_COMPACT_PLAN=0057 just compact-functions-freeze .artifacts/新しいsnapshot
IDEALIS_COMPACT_PLAN=0057 just compact-functions-compare <before> <after> <arm64-java> <jar> <jre-archive> evals/runs/新しい比較
just compact-functions-kernel <before> <after> evals/runs/新しいcriterion
just compact-functions-summary <比較run> <criterion-run> evals/runs/新しいsummary
```

snapshotはnative/Wasm、Criterion executable、4 crateのsource archiveとhashを持つ。比較runは実行したdriver source、入力、raw応答、manifestを保存する。0056の比較helperを再利用し、plan IDを明示する。検証runへbefore/A/Bのsource archiveも保存する。`.artifacts`のbinaryはcommitへ含めず、同じhashのsnapshotを保管した環境でbefore/afterを再実行する。

## 採用判断

Bを採用する。channelのnative並列探索で、preparedと起動込みの両方に改善が見えた。functionには小幅な後退が残る。全モデルの高速化やWasmの課題解消とは扱わない。

式評価の変更は今回へ含めない。Wasmでは値コピー以外の費用が残り、次は式評価とhash生成を別のplanで測る。公開IRと論理workを変える最適化や、探索範囲を減らす最適化は導入していない。
