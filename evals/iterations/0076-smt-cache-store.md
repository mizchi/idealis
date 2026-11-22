# 0075/0076: check cacheの往復費用とengine側のstore

0074の考察では、Criterionで測ったのがnativeのRust APIだけで、JSON protocolでsnapshotを往復させる費用を測っていないと指摘した。[0075](../plans/0075-smt-cache-transport.md)でその費用を測り、往復が費用の大半を占めていたので、[0076](../plans/0076-smt-cache-store.md)でcacheをengine側に保持するstoreを追加した。storeは採用する。snapshotの経路は残し、意味も費用も変えていない。

## 条件

問い合わせは32変数の整数chain（0074の`prepared-32`）。cacheには、別のsourceのentryをN個（N=0, 64, 512, 4096）入れる。

- native: Criterion 0.8.2、20 samples、warmup 1秒、measurement 2秒、median。`Engine::dispatch`と、応答の`serde_json::to_vec`を含む。
- Wasm: node 24のTS loader（`createWasm().request`）を通す。JSON.stringify・TextEncoder・dispatch・TextDecoder・JSON.parse・応答の検査を含む。20 samplesで、1 sampleは50 ms以上。
- storeのmissは、開く処理を時間に含めず、1回の要求だけを測る。
- 同じbinaryで開発・確認の2 runずつ。Apple M5、専用ホストではない。

## 0075: snapshotの往復

[0075の比較](../runs/0075-smt-protocol-comparison/result.json)。中央値（µs、開発run）と、cacheを使わない`check`に対する比。

| N    | native hit | native hit/check | その場のhit（Rust API） | Wasm hit | Wasm hit/check |
| ---- | ---------: | ---------------: | ----------------------: | -------: | -------------: |
| 0    |       14.2 |             0.85 |                    1.86 |     24.2 |           0.92 |
| 64   |       55.1 |             3.29 |                    1.80 |    102.8 |           3.91 |
| 512  |      356.4 |            21.30 |                    1.80 |    720.6 |          27.38 |
| 4096 |     2795.5 |           167.10 |                    1.86 |   5806.8 |         220.64 |

entryが1つ増えるごとに、nativeで約680 ns、Wasmで約1.4 µs増えた。1 entryはJSONで約193 bytes。cacheが答えを持っていても、64 entryを超えると解き直すより遅い。N=0でもprotocolのhit（14.2 µs）はその場のhit（1.86 µs）の約8倍で、要求の解析、毎回の取り込みでのentryのdigest検査、応答の直列化が加わる。planの条件（N=512のhitが両runでcheck以上）をnativeとWasmの両方で満たし、`implement-store`になった。

## 0076: engine側のstore

`cache-open`で取り込んだcacheをengineが保持する。要求には`cacheHandle`だけを渡す。`cache-export`は同じ形式のsnapshotを返し、`cache-close`で閉じる。

[0076の比較](../runs/0076-smt-store-comparison/result.json)。中央値（µs、開発/確認）。

| N    | native handle hit | native handle miss | Wasm handle hit | Wasm handle miss |
| ---- | ----------------: | -----------------: | --------------: | ---------------: |
| 0    |       4.64 / 4.54 |      21.09 / 21.01 |     9.83 / 9.71 |    31.81 / 31.17 |
| 64   |       4.66 / 4.57 |      21.43 / 21.02 |     9.89 / 9.74 |    32.10 / 32.04 |
| 512  |       4.67 / 4.59 |      21.85 / 21.23 |     9.90 / 9.86 |    33.33 / 33.88 |
| 4096 |       4.74 / 4.67 |      21.95 / 22.14 |     9.95 / 9.80 |    34.21 / 34.60 |

cacheを使わない`check`は、native 16.8/16.5 µs、Wasm 25.6/25.9 µs。

- hitはcheckの0.28倍（native）、0.38倍（Wasm）で、N=4096とN=0の比は1.010〜1.029。条件1・2を満たした。N=4096のsnapshotを往復させるhitに比べ、nativeで約600倍、Wasmで約590倍速い。
- missはcheckの1.20〜1.34倍で、条件3（1.5倍以下）を満たした。新しい探索に、keyの計算、entryのdigest、書き込みが加わる。
- snapshotの経路のケースは0075の0.977〜1.035倍（報告のみ）。
- handleのhit（4.6 µs）とRust APIのhit（1.8 µs）の差は、JSONの要求の解析と応答の直列化。

Wasmは482,031 bytesから520,271 bytes（+38,240、+7.9%）。

## 正しさ

seedから生成した60系列・2400要求（prepare・assert・push・pop・check-incremental・run-script-incremental、未宣言のassumption、予算0、容量4/16/4096）を、snapshotを往復させるengineとstoreを使うengineの両方へ送った。全応答と、各要求の後にexportしたsnapshotが一致した。hit・write・errorがそれぞれ一定数以上起きることも検査している。

ほかに次を検査した。

- 別buildのsnapshotを取り込むと`invalidated`になる。
- digestの壊れたentryがあると開く要求ごと拒否し、handleを割り当てない。
- 開いた時の容量を守る。2つのstoreは独立している。
- handleとsnapshotのfieldの併用、未知のhandle、閉じた後のhandleを拒否する。
- native/Wasmで同じ要求列の全応答が一致する。

人工故障3件（storeへのwriteを捨てる、開いた時のoptionを無視する、古い状態をexportする）はすべて検出した。

## 範囲と残り

storeはengineのメモリにあり、永続化しない。保存や共有は`cache-export`とmergeで行う。snapshotの経路との違いは2点ある。壊れたentryは開く時にまとめて拒否する。scriptの途中でerrorになった場合は、それまでに書いた新しい回答がstoreに残る。測ったのは1種類の問い合わせ（chain-32）で、ほかの問題やnativeのCLIのpipe越しの費用には外挿しない。

次の候補は、0074の考察のB（assertionをGitのcommitのような連鎖にし、assert/popを定数時間にする）。

```sh
just smt-protocol-test
just smt-protocol-compare-store <native-dev> <native-conf> <wasm-dev> <wasm-conf> evals/runs/0075-smt-protocol-comparison <new-output>
```
