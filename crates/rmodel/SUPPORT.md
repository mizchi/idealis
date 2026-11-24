# rmodelの対応チェックリスト

チェック済みは、1つのモデルを構文・型で検査し、AlloyとTLA+の2つへ写して同じ意味になる範囲。未チェックは未対応または対象外で、実装予定を約束しない。

## 対応済み

- [x] `sig`（scopeでちょうどの大きさ、1〜7）、静的な`rel`と状態の`var`。単項（`set`・`lone`・`one`・`some`）と二項（`A -> m B`）の関係（0091）。
- [x] `enum Name = A | B`: 名前の付いた、ちょうど1つずつのatom。scopeを書かず、sigと同じく量化と関係の型に使う（0093）。
- [x] `fact`（varを参照しない設定の制約）、`init`、引数付きの`action`。actionは連言で、`v' = 式`の代入は1つのvarに1回まで、右辺と条件にprimeを書けない。代入しないvarと`rel`は変わらない（0091）。
- [x] 式: join（単項と二項、二項同士）、`->`、`+`・`-`・`&`、`none`、単項の`#`と絶対値7以下のリテラル。論理式: `in`・`not in`・`=`・`!=`・多重度・`no`・`not`・`and`・`or`・`implies`・`all`・`some`（0091）。
- [x] `invariant`（到達できる全状態）と`reachable`（到達できる状態がある）。関係の型と多重度は全状態で成り立ち、破る遷移は起きない（0091）。
- [x] Alloyへの写し方（ralloyのSATで有界、stepsのlasso）とTLA+への写し方（rtlcで完全、`rel`はInitで列挙）。

## 照合

- [x] corpusの5モデル・20 checkで、ralloyのSAT・rtlc・Alloy 6.2.0・TLC 1.8.0の判定が、結果の前に書いた期待とすべて一致する。明示状態の深さはstepsの内側（0091）。

[評価0091](../../evals/iterations/0091-rmodel-language.md)を参照。

## 非対応・対象外

- [ ] 閉包、3項以上の関係、時相の性質の入れ子、整数の変数、module、sigの階層。
- [ ] Pklの仕様との結びつけ（0092）、Quint IRからの写し方。
- [ ] SATの有界な結果（stepsの外）を完全な検査として扱うこと。

## 制約と根拠

- [x] 未知の名前、factでのvarの参照、actionの外のprime、二重の代入、arityの違い、名前の隠蔽はエラーにする。
- [x] 2つの写し方は構文木だけを共有し、互いの出力を使わない。

[構文と型](src/syntax.rs)、[写し方](src/emit.rs)、[照合テスト](tests/engines.rs)を参照。
