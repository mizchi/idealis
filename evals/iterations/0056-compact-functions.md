# 0056: 関数値の圧縮と並列domain共有

`rtlc`の関数・record・sequenceを、共有するdomainと値の列に分けた。64要素以下の全Boolと全Int(0/1)は別のbit表現を使う。その他の型と65要素以上はScalar列を使う。公開API、IR、JSONの形式は変えていない。

## 変更した費用

以前のEXCEPTは`Vec<(Scalar, Scalar)>`のkeyと値をコピーした。変更後はdomainを共有し、値だけを更新する。整数keyの位置を確認する既存の最適化も維持する。packed値の更新とhashは要素数に比例する走査を省く。hash衝突時は全体を比較し、異なる値を同じ状態へ統合しない。

初期候補AとCは逐次で速くなったが、8 workersで後退した。root domainの`Arc`を全workerが候補更新ごとにcloneしていた。最終候補Dでは、fork時にworkerごとのwrapperを作り、mergeでcanonicalなroot domainへ戻す。rootのdomainが16個以下のとき、64 key以下のdomainだけを対象にする。forkで全domain数に比例する費用を追加しない。

membershipはkeyを含む列を作らず、値のowned iteratorで検査する。packed値と完全に一致する有限rangeだけは、domain一致後に要素数分のworkを一括計上する。budget付きworkerでは要素順を維持するため一括計上を使わない。通常のinvariant検査は重複排除後の新規状態に行う。生成数はmembershipの評価回数ではない。

## 比較条件

[plan](../plans/0056-compact-functions.md)を実装前に記録した。基準は`3c11b826`。TLCの`FcnRcdValue.select/takeExcept`をローカルcloneの`d8fb231de4e4b5d02c1b2ed1ac759c086f694021`で確認した。実行比較のTLC jarは別の固定版で、manifestにhashを保存する。

大規模比較は同じTLA+ source/configを使う。関数モデルはBoolではなく整数0/1の20要素で、1,048,576状態・20,971,521生成。チャネルはN7で、2,097,152状態・22,020,097生成。全routeのcompleteと両計数を検査した。小さい4モデルの正常・違反版も9 routeへ通し、72対照を確認する。

各routeは1 warmup後に3回測り、順序を交互・逆順に変える。wall時間は別Node processの起動、共通のTS frontend、native/Wasmの起動・検査・応答・終了を含む。Rustだけのsource解析速度を示す比較ではない。TLCとRustのAPI区間は異なるため、API区間の比率をkernelの速度比として使わない。WasmはNode上で実行し、ブラウザの速度は測っていない。

Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、arm64 Temurin 21.0.13+9と固定TLC jarを使う。NodeとJVMのheap設定は8 GiBで、厳密な共通RSS上限ではない。200 ms間隔のRSSは補助値で、実際のpeakを保証しない。専用ホストではなく負荷が変動した。sampleは除外せず、全3回と最小・最大も残す。通常測定中はbuild/test/profileを並行しない。

## 最終候補Dの測定

単位は秒、中央値（最小-最大）。各セルは同じDのrunの3 sampleから集計する。

| 実行経路        |            0/1整数関数 |             チャネル群 |
| --------------- | ---------------------: | ---------------------: |
| tla-ts 1        |    5.870 (5.578-6.907) | 17.238 (15.987-17.949) |
| TLC 1           | 13.781 (12.846-15.289) | 24.725 (24.202-26.297) |
| TLC 8           |    4.515 (4.499-4.694) |    6.507 (6.245-7.210) |
| 変更前 native 1 |    7.660 (6.987-8.023) | 17.111 (15.834-17.313) |
| 変更後 native 1 |    4.597 (4.023-5.035) | 16.037 (15.552-16.148) |
| 変更前 native 8 |    4.322 (3.836-4.544) |    6.032 (5.914-7.475) |
| 変更後 native 8 |    2.926 (2.733-3.500) |    5.999 (5.682-6.789) |
| 変更前 Wasm 1   | 11.716 (11.537-11.777) | 23.104 (22.273-23.946) |
| 変更後 Wasm 1   |    6.252 (5.981-6.292) | 21.879 (21.872-25.976) |

関数の変更前/変更後はnative 1が1.67倍、native 8が1.48倍、Wasmが1.87倍。TLCと同じ8 workersではnativeが1.54倍速かった。チャネルのnative 8は変更前比1.01倍で、改善を強く主張できる差ではない。TLC 8との差も1.08倍で、sampleの範囲は重なる。

Wasmは関数でtla-tsの1.07倍、チャネルで1.27倍の時間がかかった。今回の値表現だけで全経路の目標を達成したとは判断しない。

[Dのraw run](../runs/0056-comparison-d/result.json)と[集計](../runs/0056-summary-d/result.json)を参照。driver sourceの保存内容とraw JSONのhashも集計時に検算する。

## Criterionとprofile

Criterionはfrontend・IPC・起動を含まないprepared Rustの測定。既存14本を同じbench sourceからbuildし、20 samples、warmup 0.5秒、measurement 1秒で、before/after/after/beforeの順に実行する。表の値は2回のmean推定値の算術平均で、pooled confidence intervalではない。

単位はms。各value/membership行は4096操作のbatch、search行は小さい固定fixtureの全探索で、大規模比較とは入力サイズが異なる。counterは20,736状態、functionは4,096、channelsは4,096、ringは10,000。実行時fixtureは埋め込みfixtureとJSONとして一致した。

| kernel                             | 変更前 | 変更後 | 前/後 |
| ---------------------------------- | -----: | -----: | ----: |
| `membership/function-cube`         |  2.095 |  1.527 |  1.37 |
| `membership/parallel-channels`     |  0.969 |  0.885 |  1.09 |
| `search/counter-product/1`         |  9.556 |  9.448 |  1.01 |
| `search/counter-product/8`         |  9.579 |  9.444 |  1.01 |
| `search/function-cube/1`           |  9.699 |  6.257 |  1.55 |
| `search/function-cube/8`           |  8.065 |  6.142 |  1.31 |
| `search/linear-ring/1`             |  1.104 |  1.104 |  1.00 |
| `search/linear-ring/8`             |  1.135 |  1.097 |  1.03 |
| `search/parallel-channels/1`       | 13.186 | 12.867 |  1.02 |
| `search/parallel-channels/8`       |  7.138 |  6.932 |  1.03 |
| `values/except-function`           |  0.475 |  0.451 |  1.05 |
| `values/except-record`             |  4.052 |  3.971 |  1.02 |
| `values/intern-existing-functions` |  0.450 |  0.380 |  1.18 |
| `values/intern-new-functions`      |  0.498 |  0.476 |  1.05 |

最終Dでは14本に10%以上の後退はなかった。関数のsearchは逐次1.55倍、8 workers 1.31倍、membershipは1.37倍。チャネルとringの小さい差を有意な改善とは主張しない。[raw推定値](../runs/0056-criterion-d/manifest.json)と[集計](../runs/0056-summary-d/result.json)を保存した。

最終Dの[profile集計](../runs/0056-profile-d-retry-summary/result.json)では、関数逐次の`Arena::find`のself比率が基準の36.1%から6.7%へ下がった。チャネル逐次は式評価、SipHasher、値の検索が残る。profileはcounter M24、function N16、channels N5、ring 100,000をprepared IRから各16回実行する。CPU sampleの構成は通常のwall速度比ではない。8 workersの待機sampleも並列効率の直接値とはしない。

`0056-profile-after`は初期Aの観測で、最終Dではない。最初のD profile `0056-profile-d`は採取中のREADME/SUPPORT編集でsource固定検査が失敗したため無効とした。編集を止めた`0056-profile-d-retry`だけを最終の観測として使う。既存0024のprofile helperを再利用し、manifestに実際のsource/binary hashを保存する。

## 互換性と検証

共有domainを要求するテストは旧Vec表現でRedになり、実装後に通った。Bool/Intの区別、幅1/16/32/64/65、汎用値への変更と復帰、元の値の不変性、既存hash衝突とpacked domain衝突、workerのkey/payload remapとroot共有を検査する。

[最終の旧版照合](../runs/0056-parity-d/result.json)は172ケース・1,720コマンドで、before/after native/Wasmの全応答が一致した。0/1とBoolの幅0/1/16/32/64/65、型不一致と狭いrangeを含む。予算切れ・work・trace・エラーも比較し、boundedをcompleteへ置き換えない。[大規模比較の24組](../runs/0056-verification/parity-proof.json)も、通常のresult全体が変更前後で一致した。採用版の71 source hashとnative/Wasm binaryは、検証後にも測定用snapshot Dと一致した。

[TLC既存corpus](../runs/0056-tlc/result.json)は155件で、153一致・想定した拒否2件・不一致0。モデル評価エラー30件も合格へ読み替えず、105件の反例を検証した。[確認用seed](../runs/0056-tlc-confirmation/result.json)は32件すべて一致した。[保存再生](../runs/0056-tlc-replay/result.json)も155件で判定・計数・反例が一致し、不一致0だった。

さらに[今回の境界16ケース](../runs/0056-boundary-tlc/result.json)をTLC/native/Wasmへ直接照合し、不一致0を確認した。幅0/64/65とBool/Intの型不一致・狭いrangeを含む。これは既に旧版照合へ使った入力の回帰確認で、未知故障発見や独立した確認実験とは数えない。

`just check`、`browser`、`portable-test`、`safety-test`、`criterion-test`、eval contract tests、`runtime-build/test`、`evaluate/replay-evaluation`、`cross-test`、`async-test`、`package`を実行した。TS共通78ケースはVitestとnode:testで通り、safetyは214件、eval contractは147件。Chromiumと単独のRust/TS/Wasm consumerも通った。`just mutation`の95.4%は既存109結果の再利用で、Rust変更のmutation coverageではない。

SUPPORTの比較は大きい値実装を別の比較へ分け、impl blockを選んでJevのexcerpt上限内へ収めた。全8 crate・36比較に矛盾・未判定はなく、オフライン再生とsupportの8テストも通った。最初のformat検査とexcerpt上限による失敗は、修正後の成功と分けて[検証記録](../runs/0056-verification/result.json)へ残す。

初期候補と最終候補のrunは編集せず残す。Aは`0056-comparison`、Cは`0056-comparison-final`で、名前にfinalを含むが採用版ではない。BのCriterionは`0056-criterion`、Cは`0056-criterion-final`。採用判断はDのrunを使う。

## 再実行

```sh
just compact-functions-freeze .artifacts/新しいsnapshot
just compact-functions-compare <before> <after> <arm64-java> <jar> <jre-archive> evals/runs/新しい比較
just compact-functions-kernel <before> <after> evals/runs/新しいcriterion
just compact-functions-summary <比較run> <criterion-run> evals/runs/新しいsummary
```

snapshotにはnative/Wasm、Criterion executable、4 crateのsource archiveとhashを保存する。比較runには実行したdriver source、manifest、入力、raw応答、計時を残す。過去runの`.artifacts`はcommitへ含めず、hashを検算できる同じbinaryを保管した環境でbefore/afterを再実行する。

## 採用判断と残る課題

Dを採用する。大規模な関数探索の逐次・並列・Wasmが改善し、Criterionの全14本に10%以上の後退はなかった。A/Cの並列後退を残したまま採用しない。

Wasmでtla-tsを上回る目標は未達。チャネルの並列版も変更前からの差が小さい。次はprofileに残る式評価と汎用値の確保を対象とし、prepared実行の調整は値表現とは別のplanで検証する。対応fragmentや完全探索の範囲を狭めて速度を上げる判断はしていない。
