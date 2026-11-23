# 0095: QuintのモデルをSATでも有界に検査し、明示状態の経路と比べる

Q2の残り。[0089](0089-quint-ir-frontend.md)はQuint IRをTLA+へ写してrtlcで明示状態を探索した。ここではQuint IRをAlloy 6へ写し、ralloyのSATで有界に検査する（[0090](0090-ralloy-temporal-bmc.md)のlasso）。同じQuintモデルを2つの経路で比べる。

## 変更

- `rquint::to_alloy(ir, options)`: Quint IRからAlloy 6のテキストへ写す。0089のTLA+への写し方とは構文木の読み込みだけを共有する。
  - 状態変数は`one sig State`の`var` field。整数は`one Int`、単位のvariantだけのsum typeは`abstract sig`と`one sig`。それ以外の型は拒否する。
  - 状態変数に依存しない式（整数・集合・listのリテラル、`fold`・`foldl`、算術、比較）は、変換時にRustで評価して定数にする。集合の`fold`は昇順（TLCの集合の正規化した順、`CHOOSE`の順）で畳む。
  - `init`の代入は最初の状態の等式、`step`は`always`の中の遷移。`x' = if (c) a else b`は`(c and x' = a) or (not c and x' = b)`、`match`の代入はtagごとの選言、`nondet x = oneOf(S)`は定数の整数の集合Sの上の`some`。
  - 遷移には、何も変えないstutterを選言で加える。TLCの`-deadlock`と同じく、行き止まりで到達できる状態を変えないため。
  - 不変条件は`check { always { inv } }`。scopeは`5 Int`（-16〜15）と`8 steps`。結果は、このbit幅とstepsの内側での有界な検査。
- `just`の実験と記録: 同じAlloyテキストをAlloy 6.2.0でも検査し、判定を保存する。

## 正しさ（結果を見る前に固定）

- 0044の6モデル・12件で、ralloyのSATの判定が`fixtures/quint-generated/tlc-oracle.json`（TLCの答え）と一致すること。complete ↔ no-counterexample、invariant-violation ↔ counterexample。
- SATの反例は、ralloyの`check_instance`で検査し直す。
- 同じAlloyテキストを、Alloy 6.2.0でも12件で一致させる。
- 範囲外の構文（bool・record・map・listの状態変数、状態に依存するfold、非定数の`oneOf`）は拒否し、別の意味にしない。`Coverage.qnt`は範囲外として、拒否されることを確かめる。
- 人工故障: `if`の条件を逆にする、`oneOf`の集合から最大の要素を落とす、`match`の代入でtagと枝の対応をずらす、の3件を12件の比較が検出すること。
- 測定（判定に使わない）: 12件のSATとrtlcの時間。

## 範囲外

- 整数の桁あふれの検出（Alloyの`forbid overflow`に当たるもの）。bit幅は手で選び、記録する。
- bool・record・mapなどの値の関係への写し方、Quintのtemporal性質。
