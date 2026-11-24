# 0100: 和のsort（多態のentity）

[0099](0099-sorts-and-violations.md)のsortの型検査で、sortの違う集合を1つの集合として扱う書き方が書けなくなった。ralloyのAlloyのcorpus（43モデル）では、`Cat + Dog`（`Animal`の子）、`Account.owner in Customer + Staff`、`Triangle + Square in Polygon`のように、sortの階層や「どちらかのsort」の型として使われている（`extends`は8モデル、`abstract sig`は7モデル）。DDDの`Party = Person | Organization`のような多態のentityにも要る。mizchiと相談し、和のsortを入れる。

## 意味論

- `sort Party = Person | Org`: 既存のsig（またはenum）をまとめた和のsort。自分のatomは持たず、atomは子のatomの和。scopeは書かない。
- v1の制約: 1つのsigが属する和のsortは1つまで。和のsortの子に和のsortは置かない。
- 型: sort Aがsort Bの部分型とは、AがBと同じか、Bの子であること。
  - 和・差・積（`+`・`-`・`&`）と比較（`in`・`=`・`!=`）は、両辺の型が同じか、一方が他方の部分型であるか、両方が同じ和のsortの子であれば許す。型はその共通の型（両方を含む最も小さいsort）。それ以外はこれまでどおり型エラー。
  - joinは、繋ぐsortの一方が他方の部分型なら許す。
  - 代入は、右辺の型が変数の型の部分型なら許す。関係とactionの引数・量化の変数は、和のsortで宣言できる。
- Alloy: 和のsortは`abstract sig`、子は`extends`。TLA+: 和のsortは子のatomの集合の和。

## 正しさ（結果を見る前に固定）

- 書けなかった表現の回帰テスト: `a not in Cat + Dog`（`a: Animal`）、`Account.owner in Customer + Staff`（`owner: Account -> one Party`、`Party = Customer | Staff`）、和のsortの上の量化、和のsortで宣言した関係への子のatomの代入。いずれもparseを通り、参照評価器・rtlc・ralloyのSATで判定が一致すること。
- 拒否のテスト: 和に属さない2つのsigの混在、2つの和のsortへの所属、和のsortの子への和のsort。
- 差分テスト: 生成器に和のsortを加え（確率30%）、dev seed 5000〜5299、確認 seed 6000〜6299で、判定の食い違いが0件であること。
- 既存のcorpusと0099の回帰はすべて通ること。

## 範囲外

- 継承（`extends`）、和のsortの入れ子、1つのsigの複数の和への所属。
- 型定義の生成と実装の照合（0101）。
