# 0060: node関数の行でSYMMETRYを扱う

Rust nativeの探索を速くするため、単一のモデル値集合とscalarなnode関数に限定したSYMMETRYを実装した。複数関数の同じnodeの値を行にまとめ、行のmultisetで状態を同一視する。全N!置換を列挙せず、globalなInt/Boolと変数の役割も保持する。

## 適用条件と前提

[計画](../plans/0060-restricted-symmetry.md)を実装前にcommitした。基準は`4e838631`。TLC clone `d8fb231de4e4b5d02c1b2ed1ac759c086f694021`の`Tool.getSymmetryPerms`、`TLC.Permutations`、`TLCStateMut.fingerPrint`を読んだ。TLCが具体状態を保持し、fingerprint用の代表値を別に作る点を参照した。TLCのfingerprint byte列を移植した変更ではない。

flat moduleの`Symm == Permutations(Nodes)`、TLC import、直接割り当てた2〜64個の異なるuntyped model value集合に限定する。状態はInt/Boolと、全Nodesをdomainに持つ関数。各関数のpayloadはIntだけ、またはBoolだけ。複数の関数を独立にsortして相関を失う実装にはしていない。

SYMMETRY宣言がInit/Next/invariantを保つことは利用者の前提で、自動的なequivariance証明ではない。群の個別要素参照・CHOOSE・fold・sort等を拒否するgateと、前提の証明を区別する。node参照やnested payload、typed model value、複数群、temporal/fairness・constraints・initial/action property・coverageとの併用は対象外。nativeの複数worker要求は縮約時に逐次へfallbackする。既存TS frontendのSYMMETRY受理は追加していない。[利用例と全条件](../../docs/symmetry.md)を参照。

## 探索と互換性

同一視用のkeyと、探索・反例用の具体状態を分けた。新しいnodeには元の親から生成した値を保存する。keyのhashが衝突しても全要素を比較する。states/generated/depthは縮約後の計数で、縮約なしとの一致を期待しない。workへ群の準備とkey構築・sort用の固定費用を加え、予算切れをcompleteへ変換しない。

JSONの`SafetyProgram.symmetry`はoptional。設定しない入力と出力を維持する。Rustのstruct literalには`symmetry: None`の追加が必要。TSのcompiled program decoderにも有限群の形状検査を追加した。

RustのRedは設定の既存の拒否で失敗し、実装後に通った。TS decoderのRedも未知のwire fieldとしての拒否で失敗し、optional fieldの追加後に通った。途中のclippy型複雑性、既存Rust例のfield追加、測定driverの型エラーも保存する。追加した改行名の回帰は当初から通っており、新しい不具合検出とは数えない。

[開発corpus](../runs/0060-symmetry-development/result.json)と[文書更新後の再実行](../runs/0060-symmetry-confirmation/result.json)は固定19件。TLCの判定と取得できる状態数・生成数、native/Wasm、1/8 worker要求を比較した。初期違反ではTLCの計数が取得できず、計数の一致を主張しない。故障版の具体traceはSYMMETRYを外した元のNextへTLCで再生し、最終状態への到達も検証した。縮約なしの小さいモデルとも判定・反例の長さを比較し、両runのreport全体が一致した。同じcorpusの再実行であり、独立した新しい確認実験とは数えない。

64ノード・2値の境界はWasmで65状態・4161生成・深さ64を確認した。この境界をTLCの全64!置換へ実行比較したとは扱わない。Chromiumでも3ノード・3値の27→10状態と31生成を、host importやcross-origin isolationなしで確認した。

## 起動込みとprepared探索

最終版は7ノード・各0..5の循環counterを使う。縮約なしは279,936状態・1,959,553生成、縮約ありは792状態・5,545生成。Rust/TLCの全経路で、それぞれの完全探索と両計数が一致した。縮約あり・なしの計数を同じ値へ正規化してはいない。

1 warmup後3回を交互・逆順で測った。RustのwallはNode host、Rust frontend、別processの起動、IPCと検査を含む。TLCはJava frontendと検査を含み、既存runnerのRSS samplingも行う。Rust/TLCのkernelだけの比較ではない。Node heapとTLC heapは各8 GiB設定で、Rust heapの厳密な共通RSS上限ではない。非専用のApple M5 arm64、32 GiB、Node 24.22.2と固定arm64 Java/TLCを使った。通常測定中にbuild/test/profileを並行しない。

| 経路                        | 中央値 ms |      最小-最大 ms |
| --------------------------- | --------: | ----------------: |
| `native-plain-1`            |   673.475 |   669.209-882.638 |
| `native-symmetry-1`         |    36.539 |     35.249-40.019 |
| `native-plain-8`            |   590.802 |   483.576-624.860 |
| `native-symmetry-8-request` |    37.300 |     35.470-42.992 |
| `tlc-plain-1`               |  1559.787 | 1389.987-1713.285 |
| `tlc-symmetry-1`            |  3810.623 | 3805.079-4419.321 |
| `tlc-plain-8`               |  1204.561 | 1195.457-1460.541 |
| `tlc-symmetry-8`            |  2558.536 | 2390.334-2694.654 |

Rust逐次の起動込み所要時間は中央値で約1/18.5になった。8-worker要求も約15.8倍だが、縮約時は逐次fallbackであり、内部並列化による改善とは扱わない。TLCのこのモデルではSYMMETRYありの方が遅かった。全置換を列挙する一般的な実装と、対象形状に絞った行の同一視の比較であり、任意のTLA+での速度比へ一般化しない。

新しいCriterion targetは同じ最終候補binaryでplain/symmetryを測った。20 samples、warmup 0.5秒、measurement 1秒で、2回目は6ケースの順序を逆にした。表は2回のmean推定値の算術平均で、pooled confidence intervalではない。IRは準備済みだがcheckごとにArenaと探索を作り直す。

| モデル           | plain ms | symmetry ms |  前/後 |
| ---------------- | -------: | ----------: | -----: |
| counter-5-3-safe |    1.529 |       0.179 |   8.56 |
| counter-7-5-safe |  633.171 |       3.885 | 162.96 |
| joint-5          |    1.575 |       0.312 |   5.05 |

主モデルのprepared探索は約1/163の所要時間になった。起動込みの比とは分ける。5ノードの2つのBool関数は1,024→56状態で、異なる関数の相関を保つ。Wasmの速度を採用条件にはしていない。

[最終wall run](../runs/0060-symmetry-benchmark-final/result.json)、[最終Criterion](../runs/0060-symmetry-criterion-final/result.json)、[集計](../runs/0060-summary/result.json)へ全sampleを保存した。最初のwall runはTLCのrawログをJSON出力と取り違え、warmupで停止した。[失敗run](../runs/0060-symmetry-benchmark/error.json)も保存する。既存runnerのverifyを使うよう修正した[取り直し](../runs/0060-symmetry-benchmark-verified/result.json)の後、同名の定数・変数がPermutationsを隠す入力の拒否をテスト・修正し、最終版で全経路を再測定した。縮約エンジンの変更ではない。

## 縮約なしの14本と残る変動

同じbench sourceでbefore/after/after/beforeを実行した。初回の変更前binaryはring/channelが反復間で数倍変動し、counterの8-worker要求は平均で約13%増えた。原因は未特定で、追加診断をplanに記録して同じ条件で再実行した。初回を破棄せず、異常な対照から高速化を主張しない。

| kernel                             | 初回 前/後 | 再実行 前/後 |
| ---------------------------------- | ---------: | -----------: |
| `membership/function-cube`         |       1.03 |         1.03 |
| `membership/parallel-channels`     |       0.97 |         1.00 |
| `search/counter-product/1`         |       1.03 |         1.02 |
| `search/counter-product/8`         |       0.88 |         1.01 |
| `search/function-cube/1`           |       0.97 |         1.01 |
| `search/function-cube/8`           |       0.93 |         0.99 |
| `search/linear-ring/1`             |       2.66 |         0.97 |
| `search/linear-ring/8`             |       3.13 |         0.90 |
| `search/parallel-channels/1`       |       1.80 |         0.96 |
| `search/parallel-channels/8`       |       2.97 |         1.03 |
| `values/except-function`           |       0.99 |         0.98 |
| `values/except-record`             |       1.03 |         1.00 |
| `values/intern-existing-functions` |       0.98 |         1.06 |
| `values/intern-new-functions`      |       1.02 |         1.09 |

[初回](../runs/0060-controls/manifest.json)と[再実行](../runs/0060-controls-confirmation/manifest.json)を併記する。再実行のcounter 8は9.665→9.601 msで初回の後退は再現しなかった。一方ring 8は1.099→1.221 ms、約11%の所要時間増が残る。変更後の2回は1.111/1.332 ms、変更前は1.096/1.101 msだった。片方だけ遅く、原因を特定できていない。通常経路に後退がないとは結論しない。

最終の既存14本の実行ファイルは、この対照で使った候補版と同一hash。最後の変更はfrontendの拒否条件と測定driverの型・書式修正で、既存14本を新しいbinaryで測ったかのようには扱わない。初回・修正後・最終版のsource/binary hashを分けて保存する。

## 最終検証と採用

[最終19ケース](../runs/0060-symmetry-final/result.json)は開発・文書更新後のreportと完全一致した。hash衝突、複数関数の相関、global scalarと変数位置、部分domain、nested/node参照・混在payload、型と文字列の区別、worker fallback、coverageやtemporal等の拒否、予算切れを検査した。Permutationsを同名の定数・変数で隠す入力も、Red後に専用の拒否として検査した。

[旧版照合](../runs/0060-parity-final/result.json)は172入力・1,720コマンドでbefore/after native/Wasmの応答全体が一致した。設定なしのwire・work・budget・エラー・traceを維持する。[既存TLC155件](../runs/0060-tlc/result.json)は153一致・想定した拒否2件・不一致0、[確認32件](../runs/0060-tlc-confirmation/result.json)はすべて一致、[保存再生155件](../runs/0060-tlc-replay/result.json)も不一致0だった。モデル評価エラー30件はエラーのまま比較し、105反例を検証した。

`just check`、`browser`、`portable-test`、`safety-test`、`criterion-test`、eval contract tests、`runtime-build/test`、`evaluate/replay-evaluation`、`package`、`support-test`を実行した。rtlcは61 unit/integration、TS共通79ケースはVitestとnode:testで通った。新しいSYM wireとChromiumでの実行、独立したRust/TS/Wasm consumerも検査した。SUPPORT/Jevは8 crate・39比較でlive・offline・stagedの矛盾・未判定0。

共有テスト変更に伴い`just mutation`も実行した。既存のconformance/counterの2対象で95.4%、109件の保存結果を再利用し、新しい実行は0件、survivedは5件だった。RustのSYMMETRYや今回のTS decoderのmutation scoreを測った結果ではない。

[検証記録](../runs/0060-verification/result.json)へ、途中の失敗、最終の成功、before/候補/最終のsource archiveとbinary hash、測定時のsource一式を保存する。fixtureの書式修正はJSONとして同一で、測定済みsnapshotを上書きしない。最終source/binaryとraw応答のhashを検算する。binaryはcommitへ含めない。

限定した機能として採用する。対象モデルではnativeのwallとprepared探索の双方で有用な短縮があり、具体反例の再生と縮約なしの小モデル照合も通った。宣言の対称性は利用者の前提であり、一般のTLA+、peer参照を持つ状態、livenessまで保証を広げない。通常ringの8-worker要求の変動は未解決として残す。

## 再実行

```sh
just symmetry-test
just symmetry-prepare
just symmetry-eval evals/runs/新しい照合 <arm64-java> <tlc-jar>
just symmetry-criterion <凍結したsymmetry-bench> evals/runs/新しいcriterion
just symmetry-benchmark evals/runs/新しい比較 <arm64-java> <tlc-jar> <jre-archive> <凍結したidealis-core>
```

測定の前にnative/Wasmとbench targetをbuildし、通常測定は単独で行う。保存したsourcesと固定binaryを使い、before/afterの意味とfrontend・IPC・起動の範囲を保つ。既存14本は`compact-functions-kernel`を使う。今回の固定corpusの反復は回帰・再現で、独立した確認seedとして数えない。
