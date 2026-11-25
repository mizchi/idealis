# 0099: sortの静的な型検査と、多重度の違反の報告

[0098](0098-syntax-readability.md)の誤答6件は、すべてsortの合わない式だった。意味論v0（[docs/kernel-semantics.md](../../docs/kernel-semantics.md)）の決定（型・多重度の違反は報告する）と合わせて、次を入れる。

## 意味論の変更

- **sortの型**: 式の型は「sort Sの集合」か「sort Aからsort Bへの二項関係」。`none`はどの型にも合う。
  - 和・差・積（`+`・`-`・`&`）、比較（`in`・`=`・`!=`）は、両辺の型が同じでなければ型エラー。
  - joinは、左の最後のsortと右の最初のsortが同じでなければ型エラー。直積の型は両辺のsortの組。
  - actionの引数と量化の変数は、宣言したsortの1つのatom。enumのmemberはそのenumのsort。
  - 代入の右辺は、varの型と同じでなければ型エラー。
- **多重度の違反**: sortの型が合っていれば、遷移の結果は必ず定義域に入る。残るのは多重度。遷移の結果がvarの多重度を破るとき、その遷移は起き、違反した状態として報告する（witnessはそこへ至るtrace）。違反した状態からは遷移しない。checkは違反していない状態でだけ評価する。
  - 参照評価器: 各モデルに、checkとは別に`Declarations`（多重度の違反に到達できるか、最短の深さ）を返す。
  - TLA+: actionの条件から多重度の検査を外す。`Declarations`は全varの多重度のinvariant。違反した状態から遷移しないよう、`Next`は多重度を満たす状態からだけ遷移する。checkは`多重度 => 式`として違反した状態を除く。
  - Alloy: var fieldの多重度を宣言から外して式にする。違反した状態は最後まで止まる（stutter）。checkは`always { 多重度 implies 式 }`・`eventually { 多重度 and 式 }`。`Declarations`は`check { always { 多重度 } }`。

## 正しさ（結果を見る前に固定）

- 単体テスト: sortの合わない和・比較・join・代入、enumと別のsortの混在を拒否する。corpus（`fixtures/rmodel`・`spec/models`）はすべて型が合う（合わなければ、どこが合わないかを記録して直す）。
- 生成器はsortの型が合ったモデルだけを作る（生成したモデルはすべてparseを通る）。
- 差分テスト（0097と同じ規則、`Declarations`を含む）: dev seed 3000〜3299、確認 seed 4000〜4299。判定の食い違いが0件であること。
- 人工故障: TLA+で`Next`の多重度の前提を外す、Alloyでcheckの`多重度 implies`を外す、の2件を差分テストが検出すること。
- 読みやすさ（0098の仕組み）: 0098で取っておいた確認用のseed 2000〜（新しい生成器）でv0のprinterを読ませる。`Declarations`の問い（多重度を破る状態に到達できるか）を加える。0098のdevとはモデルが違うので、正答率の差は参考にとどめ、誤答の分類で判断する。

## 範囲外

- 部分的な演算（今のrmodelにはない）、時相の性質、文法の確定。
