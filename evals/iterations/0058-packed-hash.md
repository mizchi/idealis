# 0058: packed関数のhash計算を軽くする

64要素以下のBool/Int(0/1)関数の内部hashで、毎回のDefaultHasher構築を省く。cached domain hash、payload、型のsaltを混合する。汎用関数のhashと式評価は変更していない。

## 参照と契約

[plan](../plans/0058-packed-hash.md)は実装前にcommitした。基準は`d65c4ec0`。MurmurHash3の[公式fmix64](https://github.com/aappleby/smhasher/blob/07bb4de10a63e8cc2e1724865454eba635742383/src/MurmurHash3.cpp)を参照し、64bitのwrapping multiplicationとxor/shiftを使う。公式C++から7つのgolden vectorを生成し、Rustの結果を照合した。元コードはpublic domainで、新しいcrate依存は追加していない。

内部索引のhashを変更し、衝突時はdomain/payloadを完全比較する。公開API/IR/JSON、型の区別、canonical identity、workと予算、反例、探索対象を維持する。TLC clone `d8fb231de4e4b5d02c1b2ed1ac759c086f694021`の`FcnRcdValue.fingerPrint`も読んだ。TLCのfingerprint形式を実装する変更ではない。

## 固定した比較条件

既存14本のCriterionを、同じbench source、20 samples、warmup 0.5秒、measurement 1秒でbefore/after/after/beforeの順に測る。2回のmean推定値の算術平均を使い、pooled confidence intervalとはしない。小さいprepared fixtureを使い、frontend・IPC・起動を含まない。

大規模比較はfunction N20（1,048,576状態・20,971,521生成）とchannel N7（2,097,152状態・22,020,097生成）。tla-ts 1、TLC 1/8、before/after native 1/8、before/after Wasm 1を、1 warmup後3回、交互・逆順に測る。小さい4モデルの正常・違反版も全routeへ通す。全経路のcompleteと両計数を検算し、不完全な結果を速度比へ入れない。

主値は共通TS frontend、別processの起動、検査と応答を含むwall時間。Rust frontendだけの解析速度ではない。Node Wasmの結果をブラウザの速度として扱わない。Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、固定arm64 Java/TLCを使用する。heap設定は各8 GiBで、厳密な共通RSS上限ではない。非専用ホストの負荷変動を含め、全sampleと最小・最大を残す。通常測定中にbuild/test/profileを並行しない。

## 大規模比較

同じrunの秒数。中央値（最小-最大）を示す。チャネルの時間は参照実装も大きく変動したため、採用判断の速度比に使わない。

| 経路            |            0/1整数関数 |     チャネル群・参考値 |
| --------------- | ---------------------: | ---------------------: |
| tla-ts 1        |    5.083 (5.065-5.556) | 18.108 (16.812-24.686) |
| TLC 1           | 12.249 (12.171-12.369) | 32.643 (24.844-46.566) |
| TLC 8           |    4.196 (4.063-4.509) |   6.699 (6.457-14.364) |
| 変更前 native 1 |    3.377 (3.345-3.435) | 15.355 (14.549-16.185) |
| 変更後 native 1 |    3.191 (3.148-3.255) | 15.606 (14.069-16.984) |
| 変更前 native 8 |    2.399 (2.341-2.619) |    6.248 (5.854-7.080) |
| 変更後 native 8 |    2.280 (2.234-2.360) |    5.954 (5.406-7.448) |
| 変更前 Wasm 1   |    5.017 (4.939-5.278) | 25.020 (22.297-29.425) |
| 変更後 Wasm 1   |    4.548 (4.488-4.561) | 24.078 (23.149-30.300) |

functionは変更前比でnative 1が1.06倍、native 8が1.05倍、Wasmが1.10倍だった。同じrunのTLC 8に対しnative 8は1.84倍、tla-tsに対しWasmは1.12倍だった。この2つはfunction条件の比で、全モデルへ一般化しない。

チャネルではTLC 1が24.844-46.566秒、tla-tsが16.812-24.686秒まで変動し、before/afterの各sampleでも逆転した。ホスト条件の変動の影響が疑われる。全sampleを残し、機械的に計算した中央値の比を改善の根拠にしない。0057のチャネル測定と今回のfunction測定を同じ表へ混ぜない。

[raw run](../runs/0058-comparison/result.json)と[集計](../runs/0058-summary/result.json)を参照。driver source、raw応答のhashも集計時に検算した。

## Criterionとprofile

単位はms。value/membershipは4096操作のbatch、searchは小さい固定fixtureの全探索。

| kernel                             | 変更前 | 変更後 | 前/後 |
| ---------------------------------- | -----: | -----: | ----: |
| `membership/function-cube`         |  1.555 |  1.534 |  1.01 |
| `membership/parallel-channels`     |  0.879 |  0.881 |  1.00 |
| `search/counter-product/1`         |  9.320 |  9.661 |  0.96 |
| `search/counter-product/8`         |  9.342 |  9.297 |  1.00 |
| `search/function-cube/1`           |  6.140 |  5.691 |  1.08 |
| `search/function-cube/8`           |  6.019 |  6.007 |  1.00 |
| `search/linear-ring/1`             |  1.078 |  1.161 |  0.93 |
| `search/linear-ring/8`             |  1.079 |  1.087 |  0.99 |
| `search/parallel-channels/1`       | 12.466 | 12.240 |  1.02 |
| `search/parallel-channels/8`       |  6.711 |  6.495 |  1.03 |
| `values/except-function`           |  0.431 |  0.438 |  0.98 |
| `values/except-record`             |  3.908 |  3.943 |  0.99 |
| `values/intern-existing-functions` |  0.366 |  0.343 |  1.07 |
| `values/intern-new-functions`      |  0.455 |  0.406 |  1.12 |

関数探索の逐次は1.08倍、intern-newは1.12倍だった。10%以上の後退はなかったが、未変更の対照ring逐次は約8%、counter逐次は約4%遅くなった。原因は特定しておらず、一律に高速化したとは判断しない。

[profile集計](../runs/0058-profile-summary/result.json)は小さい既存入力をprepared IRから各16回、native 1/8へ実行した。function逐次のself比率はevalが10.0%、eval_operatorが7.8%、Arena::findが7.0%だった。channel逐次はSipHasher::writeが8.2%、RandomState::hash_one<&Value>が3.8%残り、packed hash以外のhash処理も残る。profile構成を通常のwall速度比として使わず、異なるrunのCPU比率だけで削減時間を推定しない。既存0024 helperを使い、実際のsource/binary hashを保存した。

## 互換性と検証

公式C++の7出力との一致、4,096関数でhashが重複しないこと、下位8bitのbucketが空でなく64件以内に収まることをテストした。最上位bitを含む64個の単一bit更新も区別する。この固定corpusの結果を全入力の分布保証とはしない。

BoolとIntの64要素関数でhashが一致するfixtureを追加し、型とcanonical identityが維持されることを確認する。既存のgeneric/domain衝突も完全比較する。golden vectorのテストは基準でRedになり、実装後に通った。

比較スクリプトのbaselineパス変換に誤りがあり、最初の`0058-parity`はモデル実行前に失敗した。修正後の`0058-parity-verified`を照合結果として使い、初期の失敗も残す。

[修正後の旧版照合](../runs/0058-parity-verified/result.json)は172入力・1,720コマンドで、before/after native/Wasmの全応答が一致した。予算切れ・エラー・work・traceも比較し、boundedやmodel-errorをcompleteへ置き換えない。

[大規模比較の24組](../runs/0058-verification/parity-proof.json)もresult全体が一致した。71 source hashとnative/Wasm binaryは測定用snapshotへ一致した。Criterionの実行時fixtureは埋め込みfixtureとJSONとして同じだった。[公式C++と生成出力](../runs/0058-verification/reference/manifest.json)も保存した。

[TLCの既存corpus](../runs/0058-tlc/result.json)は155件で、153一致・想定した拒否2件・不一致0。モデル評価エラー30件を合格へ置き換えず、105反例も検証した。[既存の確認seed](../runs/0058-tlc-confirmation/result.json)も32件すべて一致した。[保存再生](../runs/0058-tlc-replay/result.json)も155件で判定・計数・反例が一致し、不一致0だった。

確認seedは使用済みcorpusの回帰で、新しい独立した確認実験とは数えない。

`just check`、`browser`、`portable-test`、`safety-test`、`criterion-test`、eval contract tests、`runtime-build/test`、`evaluate/replay-evaluation`、`package`を実行した。rtlcのunit/integrationは53件、TS共通78ケースはVitestとnode:testで通る。Chromiumと単独のRust/TS/Wasm consumerも検査した。Rustのmutation scoreは測っていない。

SUPPORTのlive検査とオフライン再生は全8 crate・36比較で矛盾・未判定0。[検証記録](../runs/0058-verification/result.json)へRed、最初のパス誤り、最終の成功を分けて保存する。

## 再実行

```sh
IDEALIS_COMPACT_PLAN=0058 just compact-functions-freeze .artifacts/新しいsnapshot
IDEALIS_COMPACT_PLAN=0058 just compact-functions-compare <before> <after> <arm64-java> <jar> <jre-archive> evals/runs/新しい比較
just compact-functions-kernel <before> <after> evals/runs/新しいcriterion
just compact-functions-summary <比較run> <criterion-run> evals/runs/新しいsummary
```

snapshotはnative/Wasm、Criterion executable、4 crateのsource archiveとhashを持つ。比較runは実行したdriver source、入力、raw応答、manifestを保存する。検証runへsource archiveと公式C++の照合資料も保存する。binaryはcommitへ含めず、同じhashのsnapshotを保管した環境でbefore/afterを再実行する。

## 採用判断

候補を採用する。主目標のfunctionでnative/Wasmの通常測定とprepared逐次の改善を確認した。Criterionの対照の後退も残す。チャネルの今回の速度比は測定の変動が大きく、改善を確認したとはしない。

packed hashの変更だけでWasm全般の課題が解消したとは判断しない。式評価と他のhash計算は次のplanで分けて測る。hash衝突による状態の統合や、探索範囲・logical workの削減は導入していない。
