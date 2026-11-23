# 0075: check cacheのsnapshot往復の費用

0074のcheck cacheは、JSON protocolの`check-incremental`/`run-script-incremental`で要求ごとにcache snapshot全体を受け取り、全体を返す。0074のCriterionはnativeのRust APIだけを測り、この往復を含めていない。0074bの正しさのrunが19 MBになったのは、この往復をそのまま記録したためだった。

engine側にcacheを保持してrefと差分だけを渡す変更（以下A）を実装する前に、往復の費用を測る。このplanは測定だけで、solverとprotocolは変えない。

## 測る問い合わせ

問い合わせは0074のcache benchと同じ32変数の連鎖（`prepared-32`）。cacheは、別のsource（`(declare-const x Int)(assert (<= x k))`、k=0..N-1）のentryをN個入れたもの。N=0, 64, 512, 4096、`maxEntries`=8192。

- native（Criterion、`benches/protocol.rs`）
  - `protocol-check`: cacheを使わない`check`要求。
  - `protocol-miss-N`: N個のsnapshotを渡す`check-incremental`。対象のqueryは含まないので毎回missになる。
  - `protocol-hit-N`: N個に対象のqueryを加えたsnapshotを渡す。毎回hitになる。
  - `store-hit-N` / `store-miss-N`: 同じcacheをRust APIで直接使う。Aの下限の目安。missは複製したcacheを使い、複製の時間は測らない。
  - protocolのケースは`Engine::dispatch`と、応答の`serde_json::to_vec`を含む。nativeのCLIとWasmが行う処理と同じ。
- Wasm（node、`evals/experiments/smt-protocol/`）
  - `wasm-check`、`wasm-miss-N`、`wasm-hit-N`。TSのloader（`createWasm().request`）を通す。JSのJSON.stringify、TextEncoder、dispatch、TextDecoder、JSON.parse、応答の検査を含む。JSのclientが実際に払う費用と同じ。
  - 各ケースはwarmupの後に20 samples。1 sampleが50 ms以上になる反復回数を、warmupの時に決める。1回あたりの中央値と、raw samplesを保存する。

Criterionの条件は0074と同じ（20 samples、warmup 1秒、measurement 2秒、median、95%区間）。nativeとWasmのそれぞれで、同じbinaryの開発・確認の2 runを`0075-*`へ保存する。ほかの重い処理と同時に測らない。専用ホストではない。

## 判断（結果を見る前に固定）

- Aを次に実装する条件: 両runで、N=512のhitがcacheを使わない場合以上に遅いこと。native（`protocol-hit-512` ≥ `protocol-check`）とWasm（`wasm-hit-512` ≥ `wasm-check`）のどちらかで満たせばよい。cacheが答えを持っているのに、往復のせいで解き直すより遅くなることを意味する。
- 満たさなければAは保留し、Bへ進む。
- 報告するもの: 各Nの比（hit/check、miss/check、protocol/store）、Nに対する傾きを最小二乗法で求めたentryあたりの費用（ns/entry）、entryあたりのJSONのbyte数。

結果を見てから閾値やNを変えない。
