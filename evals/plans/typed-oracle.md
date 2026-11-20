# 型付きbatchとpayloadの転送を比較する

`254f96aaf`を基準に、roracleの照合処理へ型付きbatchを直接渡す試作をevals内に作る。WITで外枠を定義し、任意のJSON値は長さ付きの型付きpayloadへ符号化する。Rustはpayloadを借用して比較・serializeし、汎用ValueとRequestの再deserializeを作らない。reportは既存と同じJSON全応答を返し、共通decoderを使う。

既存crateと既定backendは変更しない。独立したJSON/Wasmの応答をreferenceとして使い、typed側は同じOracleGroupを使う。輸送形式の正しさは文法・範囲の検査と差分/PBTで確認する。新たな定理証明の完了とは扱わない。

測定前の条件:

- JSON/native、object/native、JSON/Wasm、WIT recordとpayload、WIT buffer、native bufferを比較する。WIT生成binding、packing、size・depth等の検査、全report構築とdecoderを計時する。出力はJSONのままであることを明記する。
- 元の6入力と共有参照のない6入力を使う。30 samples、100 warmup、単独等1000/32 cases・入れ子100 iterations、開始順回転、全呼出の計時外比較を維持する。coldは別processで10回。build/test/profileとは並走しない。
- 観測64組、object key順、Unicode、数値境界、unknown/invalid/error、空・重複ID・version・寸法違いを比較する。getter、疎なarray、symbol、非列挙field、循環、不正Unicode、非有限数と負のzeroはホストで拒否する。raw JSONの重複fieldとlegacy sequenceは既存bytes APIの範囲とする。
- payloadは型・長さ・offset・UTF-8・数値・key重複/順序を検査する。入れ子と展開後の予算、canonical JSON size 16 MiB、4096 cases、256 implementationsを維持する。新形式だけの不正bufferをmatchedへ置き換えない。
- 同一requestの保持や検査結果のcache、statusだけへの応答縮小を使わない。native bufferの入力は同期呼出中に変更しない独立bufferとし、ホストadapterは共有bufferを拒否する。
- warm中央値の速度比の幾何平均1.20以上、対照で10%以上の後退なしを採用候補の目安とする。未達なら試作と判断を保存し、既定化しない。
- source/binary/toolchain hash、入力、全reference、raw sample、失敗と再生を新しいrunへ保存する。Node/Vitest、Rust fmt/clippy/test、Wasm、実ブラウザで純粋なWIT経路も確認する。

この比較を区切ってから、次の優先対象をSMTのparse/変換とsolver内部の高速化へ移す。
