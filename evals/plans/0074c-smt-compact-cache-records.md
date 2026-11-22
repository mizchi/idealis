# 0074への追記: cache snapshotの差分記録

[0074bの確認run](../runs/0074b-smt-correctness-confirmation/result.json)の`records.jsonl`は19 MB、開発runは5.7 MBで、0042の確認run（1.5 MB）より大きい。確認runの内訳は、script cacheの2 transcriptが13.2 MB、context cacheの64系列が4.2 MBで、ほぼすべてが応答ごとに含まれる成長中のcache snapshotの繰り返し。cacheは要求をまたいで持ち越すので、記録の総量は検査したcheck数の2乗で増える。

この追記は記録形式だけを変える。solver、binary、検査の内容、合否、要約は変えない。性能・正しさについての新しい仮説ではない。

## 形式

`records.jsonl`の各応答の`cache`を、同じ流れの直前のsnapshotに対する差分へ置き換える。

- 流れはscript cacheのengineごと（`scriptCache`の名前）と、context cacheの1本。context cacheは系列をまたいで同じcacheを渡すので、流れも系列をまたぐ。
- 差分は`{digest, entries, header?, added, removed}`。`digest`は再構成したsnapshot全体の、keyを整列したJSONのSHA-256。`entries`はentry数。`header`はentries以外のfield（format・version・buildDigest）で、直前と異なる時だけ記録する。`added`は新規または値の変わったentry、`removed`は消えたkey。
- 検査は従来どおり完全なsnapshotで行う。engine間・cache有無の比較は圧縮前の値で行い、圧縮は記録の直前だけに適用する。

再生は、再計算した記録を同じ関数で圧縮して保存記録と比べる。加えて保存記録の差分を順に適用してsnapshotを復元し、各`digest`と一致することを確認する。

## 確認

0074bと同じcandidate binaryで、開発seed 0x74a1・確認seed 0x74c3を新しいrun（`0074c-*`）として再実行する。seedを再利用するのは、同じ検査を別の形式で記録できることを示すためで、新しい仮説の確認には使わない。binaryのhashは0074bのmanifestと一致しなければならない。

- 要約（`summary`）が0074bの同じphaseと完全に一致する。
- 0074bの記録を新しい関数で圧縮すると、新しいrunの記録と一致する（Z3のモデルは0074bの再生と同じく無視する）。
- 差分の復元は0074bの記録のすべてのsnapshotと一致する。
- 両runを再生できる。
- 確認runの`records.jsonl`のサイズを報告する。目標は2 MB以下とし、超えても失敗として記録するだけで、形式を変えて測り直さない。

0074bのrunは上書きせず、完全なsnapshotを含む記録として残す。0074bのrunはそのcommitのsource versionでのみ再生できる。
