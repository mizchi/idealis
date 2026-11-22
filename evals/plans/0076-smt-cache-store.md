# 0076: engine側のcheck cache store

[0075の比較](../runs/0075-smt-protocol-comparison/result.json)で、snapshotの往復が費用の大半を占めていた。N=512でprotocolのhitは、cacheなしの`check`のnative 21倍、Wasm 28倍だった。entryが1つ増えるごとに、nativeで約680 ns、Wasmで約1.4 µs増える。同じcacheをその場で使うhitは1.8 µsだった。

cacheをengine側に保持し、要求ではhandleだけを渡す方法を追加する。既存のsnapshotの経路は残し、意味も変えない。

## protocol（version 1への追加）

- `cache-open {buildDigest, maxEntries, cache?}` → `cache-opened {cacheHandle, entries, invalidated}`
  - 渡したsnapshotを取り込み、snapshotの経路の最初の要求と同じようにbindする（format・version・build digestの検査、別buildのentryの破棄、容量の切り詰め）。
  - 加えて、全entryのdigestを開く時に検査する。壊れたentryが1つでもあれば要求ごと拒否し、handleを割り当てない。
- `check-incremental` / `run-script-incremental`に`cacheHandle`を追加する。
  - 指定した場合はstoreをその場で使う。応答は`cache`の代わりに`cacheHandle`を返す。
  - `cache`・`enabled`・`buildDigest`・`maxEntries`との併用はprotocol error。optionは開いた時の値を使う。
- `cache-export {cacheHandle}` → `cache-snapshot {cache}`。snapshotの経路と同じ形式。
- `cache-close {cacheHandle}` → `released`。
- cache handleは、問題のhandleとは別に採番する。未知のhandleはprotocol error、採番の上限はresource error。

## 意味の対応

開いた直後にexportしたsnapshotから、snapshotの経路で同じ要求列を実行した場合と比べる。結果・work/stats・cache stats・最後にexportしたsnapshotがすべて一致すること。違いは2点だけで、どちらも文書に残す。

1. 壊れたentryは、開く時にまとめて拒否する。snapshotの経路では、そのkeyを引いた時に初めて失敗する。
2. scriptの途中でerrorになった場合、snapshotの経路ではcacheが返らないが、storeにはerrorの前に書いたentryが残る。書かれるのは、新しく探索して得た完全な回答だけ。

## 検査

- Rust: 要求列をseedから生成し、snapshotの経路とstoreの経路の全応答を照合する（prepare・assert・push・pop・check-incremental・run-script-incremental、未宣言のassumption、予算0を含む）。
- Rust: 別buildの取り込み、壊れたentry、fieldの併用、未知のhandle・閉じた後のhandle、容量、2つのstoreの独立を検査する。
- 人工故障: storeの経路で、writeを捨てる、optionを要求ごとに無視する、exportで古い状態を返す、の3件を、上の検査が検出すること。
- TS: wireの型とdecode、native/Wasmで同じ要求列の全応答が一致すること。

## 測定と採用条件（結果を見る前に固定）

0075のbenchに次のケースを加え、同じbinaryで開発・確認の2 runを、native（Criterion）とWasm（node）で保存する。問い合わせ・N・条件は0075と同じ。

- `handle-hit-N`: N個のfillerと対象queryを持つstoreに、handleで問い合わせる。
- `handle-miss-N`: N個のfillerを持つstoreを開き（時間に含めない）、1回問い合わせる。
- Wasmは`wasm-handle-hit-N` / `wasm-handle-miss-N`。missは、1 sampleを1回の要求にする。

採用条件は次の3つ。両runで満たすこと。

1. 全Nで`handle-hit-N`が`protocol-check`（Wasmは`wasm-check`）以下。
2. `handle-hit-4096` / `handle-hit-0` が1.5以下。費用がNに依存しないこと。
3. 全Nで`handle-miss-N`が`protocol-check`の1.5倍以下。

snapshotの経路のケースは、0075の中央値との比を報告するだけにする。測った時が違うので合否にしない。Wasmのサイズも報告する。

満たさないケースは後退として記録し、閾値を変えない。
