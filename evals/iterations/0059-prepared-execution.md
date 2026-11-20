# 0059: 定数cacheを採用し、型別命令と並列照合の試作を棄却する

Data定数のprepared化、型別命令、並列の訪問済み照合の順に実装して測った。定数cacheを最終候補に残し、後退した2案は本体から除いた。公開API/IR/JSONと探索範囲は変えない。

## 参照と固定条件

[plan](../plans/0059-prepared-execution.md)を実装前にcommitした。基準は`5c272b9a`。TLC clone `d8fb231de4e4b5d02c1b2ed1ac759c086f694021`の`LazyValue`と`FastTool`を読み、状態依存式のcacheで意味が変わる条件を確認した。型別命令では`Integers`と`Tool`の関数適用も参照した。

各段階は既存14本の同一Criterion sourceで比較する。20 samples、warmup 0.5秒、measurement 1秒、before/after/after/before。表は2回のmean推定値の算術平均で、pooled confidence intervalではない。value/membershipは4096操作、searchは固定した小さいfixtureの完全探索。value/membershipの単独kernelは従来どおりprepared表を使わず、searchは通常のcheckから表を作る。

大規模比較はfunction N20とchannel N7を9経路へ通す。1 warmup後3回、交互・逆順で測る。共通TS frontend、別processの起動、検査と応答を含み、prepared Rustの費用と分ける。通常測定中にbuild/test/profileを並行しない。Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、固定arm64 Java/TLC。各heapは8 GiB設定で、同じ厳密なRSS上限ではない。

## 段階ごとの実装と採否

Data式のアドレスからslotを作り、Arenaごとに最初の成功結果を保持する。programはimmutableな表だけを共有し、workerは独立したcacheを持つ。初回は従来のdata変換を行い、2回目からValue全体のhashを省く。未評価のDataを先に変換せず、失敗・状態依存式・check間の結果をcacheしない。未登録の動的な式は従来のdata変換へ戻す。workerのRefはmergeでremapする。

定数cacheはチャネルのprepared探索で逐次1.065倍、8 workerで1.028倍だった。10%以上の後退はなく、軽いscalar探索はほぼ横ばいだった。これは全式の定数畳み込みではない。

型別命令の試作は、Literal/Read/Bound/Old/Dataの葉operandを持つ算術・比較・否定・関数選択だけを事前に変換した。式のアドレスから命令を探し、型を検査して実行する。未対応の式は既存interpreterへ戻し、左辺の型検査・右辺の評価順、overflow、primeとworkを保つ。全IRを命令列へlowerするVMではない。

型別命令は定数cache版に対しcounter逐次が約32%、ring逐次が約23%、チャネル逐次が約14%遅くなったため棄却した。検索と追加dispatchを含む設計の結果で、型別実行自体の限界とは判断しない。原因の費用は個別計測していない。再検討には式から命令へのhot pathの検索を省く設計が必要になる。

並列照合は定数cache版から試した。worker deltaを従来の順にmergeした後、512候補以上のbatchでstate hashと訪問済みlookupを並列化する。既知のIDを順序どおりのacceptへ渡し、未知はaccept時に再照合する。constraints、重複、hash衝突の完全比較、budgetと反例の順序を維持する。完全な並列acceptやsharded storeではない。

並列照合はチャネル8 workerで1.069倍だったが、function 8 workerが約47%遅くなったため棄却した。型別命令と組み合わせた測定ではない。両試作のsource archive、binary hash、Red/Greenと全測定値を残し、本体へruntime設定や未使用の実装を残さない。wide batchの挙動テストは回帰として残す。

## 14本の段階別比較

比率は前/後で、1未満が後退。段階1の前は基準、段階2・3の前はそれぞれのrunで測った定数cache版。同じ版の値でもrun間で変動するため、異なるrunを直接割らない。

| kernel                             | 定数cacheの変更前→後 ms | 定数cache 前/後 | 型別命令 前/後 | 並列照合 前/後 |
| ---------------------------------- | ----------------------: | --------------: | -------------: | -------------: |
| `membership/function-cube`         |             1.552→1.516 |            1.02 |           0.97 |           0.94 |
| `membership/parallel-channels`     |             0.889→0.920 |            0.97 |           0.98 |           1.03 |
| `search/counter-product/1`         |             9.566→9.532 |            1.00 |           0.76 |           1.03 |
| `search/counter-product/8`         |             9.525→9.516 |            1.00 |           0.83 |           0.97 |
| `search/function-cube/1`           |             5.736→5.674 |            1.01 |           0.99 |           0.99 |
| `search/function-cube/8`           |             5.720→5.621 |            1.02 |           0.96 |           0.68 |
| `search/linear-ring/1`             |             1.098→1.103 |            1.00 |           0.81 |           0.98 |
| `search/linear-ring/8`             |             1.111→1.105 |            1.01 |           0.84 |           0.94 |
| `search/parallel-channels/1`       |           12.004→11.269 |            1.07 |           0.88 |           1.01 |
| `search/parallel-channels/8`       |             6.666→6.483 |            1.03 |           0.90 |           1.07 |
| `values/except-function`           |             0.451→0.466 |            0.97 |           1.00 |           1.00 |
| `values/except-record`             |             3.974→4.017 |            0.99 |           1.01 |           1.01 |
| `values/intern-existing-functions` |             0.347→0.337 |            1.03 |           0.99 |           0.99 |
| `values/intern-new-functions`      |             0.420→0.429 |            0.98 |           1.01 |           0.97 |

[定数cache](../runs/0059-s1-criterion/manifest.json)、[型別命令](../runs/0059-s2-criterion/manifest.json)、[並列照合](../runs/0059-s3-criterion/manifest.json)に4回のCriterion出力を保存した。型別命令のcounter 8、ring 1/8、チャネル1/8にも10%以上の後退がある。並列照合のfunction 8も10%以上の後退で、不採用理由に含める。

## 大規模モデルの最終比較

最終候補は定数cacheだけ。functionは1,048,576状態・20,971,521生成、channelは2,097,152状態・22,020,097生成で、全9経路が完全探索と両計数に一致した。小さい4モデルの正常・違反版も全経路で72件を検査した。

同じrunの起動込み秒数。中央値（最小-最大）を示す。warmupは中央値に含めない。

| 経路            |            0/1整数関数 |             チャネル群 |
| --------------- | ---------------------: | ---------------------: |
| tla-ts 1        |    5.843 (5.787-6.088) | 16.374 (16.254-16.579) |
| TLC 1           | 14.183 (13.658-14.271) | 24.455 (23.647-24.634) |
| TLC 8           |    4.489 (4.424-4.590) |    6.743 (6.624-7.574) |
| 変更前 native 1 |    3.889 (3.875-4.128) | 14.075 (13.933-14.358) |
| 変更後 native 1 |    3.882 (3.873-3.939) | 13.463 (13.303-13.630) |
| 変更前 native 8 |    2.507 (2.478-2.514) |    5.887 (5.605-6.028) |
| 変更後 native 8 |    2.485 (2.406-2.713) |    5.432 (5.402-6.251) |
| 変更前 Wasm 1   |    5.301 (5.272-5.314) | 21.191 (20.831-21.245) |
| 変更後 Wasm 1   |    5.200 (5.133-5.202) | 19.308 (19.233-19.426) |

チャネルの前/後はnative 1が1.05倍、native 8が1.08倍、Wasmが1.10倍。逐次とWasmは3回とも変更後が速く、計測範囲も重ならなかった。native 8は最終反復で逆転し、両版の範囲も重なるため、安定した1.08倍の改善とは扱わない。TLC 1とtla-tsの今回の幅は0058ほど大きくないが、非専用ホストでの結果という制約は残る。

functionはnative 1が1.00倍、native 8が1.01倍、Wasmが1.02倍で、大幅な改善はない。チャネルの変更後native 1は同じrunのtla-ts比1.22倍、native 8はTLC 8比1.24倍。チャネルのWasmは19.308秒で、tla-tsの16.374秒より約18%遅く、Wasmの目標は未達。Node Wasmの数値をブラウザ速度へ一般化しない。

[全sample](../runs/0059-comparison/result.json)と[集計](../runs/0059-summary/result.json)を参照。集計時に72 sampleと72 controlのraw応答のhashを検算した。

## 最終profileと残る費用

[profile集計](../runs/0059-profile-summary/result.json)は、小さい既存4モデルをprepared IRから16回、native 1/8へ実行した。counter逐次のself比率はeval_operatorが20.6%、evalが17.4%、StateStore::findが12.3%。channel逐次はeval_operatorが11.8%、evalが6.4%、SipHasher::writeが4.6%、data_expressionが3.8%だった。式評価と索引の費用は残る。

今回のchannelのsample一覧にはValueを引数に取るhash_oneのsymbolが現れなかった。費用がゼロという保証ではなく、Data定数をslotへ変えた後の観測。usizeのhash_oneは3.2%残り、table lookupを無料とは扱わない。異なるprofileのCPU比率からwall時間の削減量を推定しない。8 workerの待機sampleも速度比へ換算しない。

既存0024 helperを再利用し、manifestのplan名と今回の0059を区別する。実際のsource/binary hash、stripを無効にしたprofile専用build、raw sampleを保存した。profile採取は通常の速度測定の後に単独で行った。

## 互換性と検証

定数cacheは、未評価分岐の不正値、失敗後の再評価、別Arenaの独立性、未登録の動的な式、worker同士のRef衝突とmergeでの補正をテストした。型別命令では左辺の型検査から右辺の評価へ進む順序、overflow、prime、Bound/Old、fallbackを比較した。並列照合ではhash衝突と同batchの重複を検査した。wide batchの制約・予算・エラー・最短反例の挙動テストは最終版にも残す。

[旧版照合](../runs/0059-parity/result.json)は172入力・1,720コマンドで、before/after native/Wasmの応答全体が一致した。work、予算切れ、エラー、traceも比較し、boundedやmodel-errorをcompleteへ置き換えない。[大規模24組の照合](../runs/0059-verification/parity-proof.json)もresult全体を比較する。最終版のnative/Wasm/Criterion executableは段階1と同一hashで、棄却した試作を混ぜていない。最終snapshotの全source hash、実際のnative/Wasm binary、Criterion fixtureも検算する。

[TLC corpus](../runs/0059-tlc/result.json)は155件で、153一致・想定した拒否2件・不一致0。モデル評価エラー30件をエラーのまま比較し、105反例も検証した。[確認用32件](../runs/0059-tlc-confirmation/result.json)はすべて一致し、[保存再生155件](../runs/0059-tlc-replay/result.json)も不一致0だった。確認seedは既に使ったcorpusの回帰で、新しい独立した確認実験には数えない。

`just check`、`browser`、`portable-test`、`safety-test`、`criterion-test`、eval contract tests、`runtime-build/test`、`evaluate/replay-evaluation`、`package`、`support-test`を実行した。Rustのunit/integrationは58件、TS共通78ケースはVitestとnode:testで通る。Chromiumと単独のRust/TS/Wasm consumerも検査した。Rustのmutation scoreは測っていない。

SUPPORTのlive検査とオフライン・staged検査は全8 crate・37比較で矛盾・未判定0。最初の通常checkは追加したMarkdownの表の書式で失敗し、整形後に全検査をやり直した。型別命令のテストhelperの型修正を含め、途中の失敗と最終の成功を[検証記録](../runs/0059-verification/result.json)へ分けて保存する。

## 再実行と次の設計

```sh
IDEALIS_COMPACT_PLAN=0059 just compact-functions-freeze .artifacts/新しいsnapshot
just compact-functions-kernel <before> <after> evals/runs/新しいcriterion
IDEALIS_COMPACT_PLAN=0059 just compact-functions-compare <before> <after> <arm64-java> <jar> <jre-archive> evals/runs/新しい比較
just compact-functions-summary <比較run> <criterion-run> evals/runs/新しいsummary
```

snapshotはnative/Wasm、Criterion executable、4 crateのsource archiveとhashを持つ。検証記録へbefore・3試作・最終版のsource archiveとmanifestを保存した。棄却した試作は隔離checkoutへ復元して比較する。binaryはcommitへ含めず、同じhashのsnapshotを保管した環境で再実行する。全sampleと失敗候補を残し、raw runは上書きしない。

定数cacheを採用する。型別実行を再検討するなら、評価入口から準備済み命令を直接参照し、式ごとのhash lookupを省く案を別planで測る。訪問済み照合の並列化は、未知候補の再照合とbatchごとの同期を含めて設計し直す。今回の試作を、全IRのVMやsharded state storeまで評価した結果とは扱わない。
