# Rust契約の生成とNode native bindingの評価

roracleのRust型からTS宣言を生成し、Wasmとnapi-rsで同じ照合処理を呼べるようにした。型生成は採用する。native経路は[事前の基準](../plans/native-oracle.md)に達しなかったため、明示選択する実験に留める。workspaceの既定はWasmのまま。

## 実装と比較範囲

`typescript` featureでrequest・observation・reportの宣言を生成する。napi-rsの関数宣言も同じ`OracleRequest`を参照し、生成物の差分をCIで検出する。通常のroracleはserde/serde_jsonだけに依存し、ts-rsやNode bindingを必要としない。TS宣言で表せない未知field拒否や件数などはRustが検査する。

native JSONは文字列入力、native objectはJSON互換objectの入力を使う。両者の出力はJSON文字列と共通decoderを通す。object入力の検査とnapi-rsのcloneも時計内に含む。この経路はJSONの処理をすべて省くものではない。

Value境界はbytesと同じRequest・OracleGroupを呼び、canonical serializationのsizeと入れ子を制限する。過深入力を拒否するRedでは、bytes parserを迂回してcheckedになった。深さ検査と再帰dropを避ける処理を追加し、4096段の拒否も確認した。初期の重複field対照はJSのJSON.parseでfieldを失っていたため、raw JSONをWasmへ渡す対照に修正した。

## 固定6入力のwarm測定

Apple M5、Darwin 25.6.0、arm64、Node v24.22.2、rustc 1.98.0。release build済みのnative/Wasmを使い、build・test・profileを並走させず測定した。各入力30 samples、100 warmup、開始順を回転する。単独・Unicode・mismatch・unknownは1000 iterations、32 cases・入れ子256要素は100 iterations。計378000回の全reportを時計の外で保存referenceへ照合した。

値は1呼出あたりの中央値、単位はµs。速度比はWasm時間をnative時間で割ったもの。requestのcodec・照合・report生成・共通decoderを含み、snapshot生成とSUTの実行は含まない。

| 入力          |    Wasm | native JSON | native object入力 | JSON速度比 | object速度比 |
| ------------- | ------: | ----------: | ----------------: | ---------: | -----------: |
| 単独          |   3.772 |       3.440 |            11.063 |      1.096 |        0.341 |
| 32 cases      |  82.605 |      81.401 |           294.343 |      1.015 |        0.281 |
| 入れ子256要素 | 216.556 |     210.803 |           864.560 |      1.027 |        0.250 |
| Unicode       |   5.774 |       5.615 |            15.913 |      1.028 |        0.363 |
| mismatch      |   3.853 |       3.665 |            11.723 |      1.051 |        0.329 |
| unknown       |   3.729 |       3.507 |            10.778 |      1.063 |        0.346 |

JSON速度比の幾何平均は1.047、objectは0.316。採用条件の1.20以上を満たさない。objectは検査と変換を含む全体で約3.17倍遅い。内訳のprofileは取っていないため、費用を特定のnapi-rs処理だけへ帰属させない。JSONの小さい差に統計的な有意性は主張しない。

## 起動を含む測定

各経路を新しいprocessで10回測った。値は中央値、単位はms。OSのfile cacheは消していない。

| 経路              | 読込み・初期化・初回request | Node起動・module importも含むprocess全体 |
| ----------------- | --------------------------: | ---------------------------------------: |
| Wasm              |                       2.656 |                                   30.111 |
| native JSON       |                       0.908 |                                   28.419 |
| native object入力 |                       1.076 |                                   27.719 |

nativeの初期化は短いが、この小さな照合処理ではNode全体への差は約2 ms。これはroracleの輸送境界の比較で、TLA+/SMT探索器のnative速度や、他OSの結果ではない。

## 再生と検証

主測定は`measurement-02`。初回の`measurement-01`はsource収集に実行時依存の`src/trace.ts`がなかったため、収集先だけ補って再測定した。入力・予算・採用基準は変えていない。初回のJSON/object速度比の幾何平均は1.043/0.319で、採用判断は同じだった。両runを編集せず保存した。

replayで18組の全report、source/binaryのhash、540 warm samplesと30 cold samplesの集計を再検査した。保存reportを改変してfixtureのhashも更新した対照は、全report比較で失敗した。保存sourceを改変した対照はhash検査で失敗した。このhashは改変検知であり、外部から渡されたarchiveの真正性を証明するものではない。

検証結果:

- `just check`: lint・型検査、Node/Vitest各79件が成功。
- `just wasm-test`: 実Wasmの共有契約をNode/Vitest各23件で検査。
- `just oracle-native-test`: Node/Vitest各10件、数値境界・特殊キー・64通りの観測・拒否入力・生成型を確認。workspace共有8件もnativeの両経路で成功。
- Rustのfeature付き全target tests、fmt/clippy、既存`just oracle-test`とruntime検査が成功。
- `just package`: npmと8 crateの単独consumerが成功。既定のroracle依存にts-rs/napiを含まないことも確認。
- 既存評価の保存反例と6 schedulesを再生し、故障版の再現と修正版の成功を確認。
- 既存TS mutation対象は104/109で95.4%。native Rustの変異検出率ではない。
- Jevの68比較は矛盾・未評価0件。offline/staged gateと検査スクリプト8件も成功。

最後にevalsの索引へ加えた段落の再検査は、JevのHTTP 402で全件request-failedになった。失敗した記録を保存し、任意の索引追加を取り下げた。検査対象が同一の成功済み判定を戻し、現在のworking treeとindexに対するoffline gateで鮮度を確認した。未回答の記録を成功へ置き換えてはいない。

Wasm/nativeは同じRust照合ロジックを使うため、この差分検査だけではRust側に共通する誤りを検出できない。`idealis-core`全体のDTO生成や探索器のnative bindingは未対応。[実行手順](../roracle-native/README.md)と[保存証拠](../runs/native-oracle/evidence.json)を参照。
