# Rust契約の生成とroracleのnative bridge

roracleのRust request/reportを型の原本にし、TS宣言を生成する。既存のJSON/Wasm ABI、status、全report、拒否条件を維持する。型生成は開発用featureへ分け、通常のRust/WasmにNodeやtypegenの依存を要求しない。

試作bindingは`evals/roracle-native/rust`に置き、同じRust実装をnapi-rsから呼ぶ。native JSONとnative object入力をWasm JSONと比較し、エンジン差と変換費用を分ける。出力は両native経路ともJSONと共通decoderを使い、特殊keyをproperty setterへ渡さない。object入力はplainなJSON dataに限定し、accessor・非有限数・負のzero・不正Unicode・循環等を事前に拒否する。nativeがない環境で黙ってWasmへfallbackしない。Wasmの既存23契約は引き続き実Wasmを直接実行する。

測定前の条件:

- 単独・32ケース・入れ子256要素・Unicode・mismatch・unknownの固定入力を使う。
- 同じ入力の全reportを照合し、statusだけでは合格させない。空・不正version・不正観測・寸法違いと型変換エラーを検査する。
- BigInt、undefined、NaN、負のzero等のJS観測は既存snapshotへ通す。native objectはJSON互換の入力を対象とし、raw JSONの重複field等はbytes経路で比較する。
- warmは初期化を除き、30 samples、100 warmup、単独1000/32ケース100/入れ子100/他1000 iterationsを使う。開始順を回転し、全呼出の結果を事前に確定したreportと計時外で照合する。各呼出を計時し、sampleごとの合計nsと件数を保存する。既存snapshot生成とSUTの実行費用は含めない。build/test/profileとは同時に測らない。
- require/compile/instance生成を含むcoldは別processで各10回測定する。Rust本体の探索速度と扱わない。
- native経路はWasmに対するwarm中央値の幾何平均1.20倍以上を候補とし、対照入力に10%以上の後退があれば全体の既定化を見送る。初回の対応OSは実際に検査したhostに限定する。
- 再生は保存入力・期待report・生sample・source/binary digestを検査する。壊れたreportの対照を含める。結果に応じて採用・明示選択・見送りを区別する。

Node/Vitestの同じ契約、Rust fmt/clippy/tests、Wasm、生成物の鮮度、共通workspace比較、配布consumerとSUPPORT/Jevを検査する。初回はroracleの契約とbindingを対象とし、idealis-core全体のDTO生成やnative bindingへ対応済みとは数えない。
