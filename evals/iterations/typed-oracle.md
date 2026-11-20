# WITと型付きpayloadによるoracle転送の比較

WIT record、WIT buffer、native bufferを試作した。Rustがpayloadを借用して同じOracleGroupへ直接渡し、汎用Value生成とRequestへの再deserializeを省く。それでもJS objectからpackingする全経路はJSONより遅かった。公開経路への採用は見送り、既定のWasmを維持する。次の優先対象はSMTの高速化とする。

## 実装した範囲

WITはversion・ID・case・観測variantとpayloadを定義し、wit-bindgenとjcoでRust/TSのbindingを生成する。Nodeは同じ試作コアへbufferを渡す。JSON/native、object/native、JSON/Wasmも同じ入力と全reportで比較した。WITの原理や他のbinding全般が遅いという結果ではなく、今回のproducerがJS objectである試作の結果。

payloadは長さ付きnodeのforestで、RustはUTF-8、数値、範囲、key順/重複、size/depthと展開後の予算を検査する。数値には既存のNumberの意味を保つためJSONの数値表記を使う。数値leafのparseは残る。出力もJSON全reportのままで、全入出力をbinary化した試作ではない。[仕様と実行手順](../roracle-wit/README.md)を参照。

既存crateとAPIは変更していない。新しいcrateとNode bindingはevals内の未公開実験。WIT componentのhost importはなく、jcoの生成JSはNodeのAPIに依存しない形にした。実Chromiumでも140個の全reportを既存JSON/Wasmへ照合した。

## 同じ全requestの速度

[事前計画](../plans/typed-oracle.md)を固定し、Apple M5、Darwin arm64、Node v24.22.2で測定した。元の6入力と共有参照のない6入力、30 samples、100 warmup、開始順回転。単独等は1000、32 casesと入れ子は100 iterations。packing・binding・Rust検査/照合/全report・共通decoderを含み、全応答の比較は時計の外。build/test/profileは並走させていない。

下表は元の6入力の中央値、単位はµs。共有参照のない6入力も生sampleへ保存し、採用判定に含める。

| 入力      | JSON/Wasm | native JSON | native object | WIT record | WIT buffer | native buffer |
| --------- | --------: | ----------: | ------------: | ---------: | ---------: | ------------: |
| 単独      |      3.41 |        3.15 |          9.71 |      13.13 |      11.92 |          7.06 |
| 32 cases  |     78.97 |       76.88 |        278.68 |     244.79 |     234.72 |        212.93 |
| 入れ子256 |    206.08 |      200.49 |        822.64 |    1126.32 |    1123.89 |       1045.86 |
| Unicode   |      5.27 |        5.03 |         14.08 |      18.24 |      17.31 |         12.24 |
| mismatch  |      3.50 |        3.36 |         10.53 |      13.50 |      12.52 |          7.89 |
| unknown   |      3.41 |        3.27 |         10.01 |      13.56 |      12.13 |          7.22 |

native JSONをbaselineにした速度比の幾何平均は、WIT recordが0.248、WIT bufferが0.263、native bufferが0.367。時間では約4.04/3.80/2.72倍となり、事前の1.20倍以上という採用目安を満たさない。JSON/Wasm比でも0.258/0.274/0.383だった。

別processで各10回測ったcoldの中央値は、JS moduleのimport・初期化・初回requestを含め、JSON/Wasmが4.72 ms、native JSONが3.17 ms、WIT recordが7.85 ms、WIT bufferが7.47 ms、native bufferが3.64 ms。Node起動を含めると約26-31 ms。前回のcoldとは計時境界が異なるため倍率を作らない。

## packing済み入力との違い

全requestを測った後、3入力を20 samples、100 warmup、同じiterationsで区間測定した。binding呼出の区間は事前にpackingし、返るJSON全reportを照合する。共通decoderの時間は含まない。独立した中央値の和は厳密な内訳ではない。

| 入力      | JS packing | native呼出 | WIT record呼出 | WIT buffer呼出 |
| --------- | ---------: | ---------: | -------------: | -------------: |
| 単独      |       4.53 |       1.41 |           7.41 |           5.04 |
| 32 cases  |     164.84 |      27.54 |          72.26 |          49.02 |
| 入れ子256 |     953.94 |      99.57 |         189.86 |         187.99 |

この試作ではJS側のpackingが大きい。property descriptor検査・keyのsort・fieldごとのTextEncoder・長さやoffsetの書込み・buffer確保を行っている。Rust側の汎用Valueを外しても、その前にJSから表現を作り直す費用が発生した。producerが最初から型付きbufferを作る、またはRust内に状態を保持して小さい操作を渡す設計なら別の結果になり得るが、今回は実測していない。

## 正しさと失敗の記録

観測64組、fixture、特殊keyとUnicode、数値境界と決定的な200候補、3箇所のdepth境界、size上限、raw bufferの切断/余剰byte/不正tagをNode/Vitest各7契約で比較した。元のJSON/Wasmをreferenceにして全reportを照合し、unknown/invalid/errorも保持する。同じOracleGroupを使うため、共通の照合ロジックの誤りまで検出する試験ではない。

depthのRedでは、実装側観測がactual配列内にある分の1段を数えておらず、123段のvalueを余分に受理した。base depthを分けて修正した。measurement-01/02の前には測定器のTS型エラーが残ったままJSがemitされ、測定が動いていた。両runを残し、型修正後はprecheckを失敗時に止め、成功したmeasurement-03を本文の値とした。改善した数字を選ぶための再測定ではない。

再生は保存したruntime、binding、binary、sourceとhashを確認し、72個の全report、2160 warm samples、60 cold samplesと判定を再計算した。保存referenceを改変しfixtureのhashも更新した対照は全report比較で拒否し、runtime改変の対照はhash検査で拒否した。Rust fmt/clippy/tests、基本検査のNode/Vitest各79件、既存Wasm各23件、runtime、評価の保存反例が成功した。既存TS mutation対象は104/109の95.4%で、109件をcacheから再利用した。今回のRust試作の変異検出率ではない。チェックリストのoffline/staged検査も68比較が成功した。live JevはAPIキー未設定として警告してスキップし、変更していないcrateの既存判定の鮮度を確認する。

[保存証拠](../runs/typed-oracle/evidence.json)に全runと失敗、生成物とtoolchainのlockを残す。WITの性能調整を続けるより、次はSMTのparse/変換とsolver内部を分けて測る。
