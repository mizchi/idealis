# 0095: QuintのモデルをSATでも有界に検査し、明示状態の経路と比べる

[plan](../plans/0095-quint-sat.md)のとおり、Quint IRをAlloy 6へ写す`rquint::to_alloy`を作り、ralloyのSATで有界に検査した（5 bitの整数、8 steps）。0089のTLA+への写し方とは、IRの読み込みだけを共有する。

- **一致**: 0044の12件のうち10件で、ralloyのSATの判定がTLCの答えと一致した。同じAlloyテキストをAlloy 6.2.0で検査しても10件で一致した。SATの反例はすべて`check_instance`で検査し直した。
- **拒否**: SequenceFoldの2件は、listの畳み込みが312を作り、ralloyの整数（最大5 bit、-16〜15）に収まらない。変換が「outside the 5-bit range」として拒否し、桁あふれした値で検査しない。planは12件の一致を期待していたので、これはplanからの差として記録する。`Coverage.qnt`は、record・map・listを使うので範囲外として拒否される。
- **故障**: `if`の条件を逆にする（Counter broken）、`oneOf`の集合から最大の要素を落とす（Choice broken）、`match`のtagと枝の対応をずらす（Match safety）の3件を、12件の比較が検出した。

## 時間（release、5回の中央値、負荷平均10）

| モデル     | 性質   | SAT（ms） | rtlc（ms） |
| ---------- | ------ | --------: | ---------: |
| Counter    | safety |   228.817 |      0.006 |
| Counter    | broken |    27.483 |      0.005 |
| SetFold    | safety |     7.080 |      0.040 |
| SetFold    | broken |     3.482 |      0.038 |
| Choice     | safety |    22.116 |      0.005 |
| Choice     | broken |    12.600 |      0.003 |
| Match      | safety |     1.534 |      0.019 |
| Match      | broken |     0.514 |      0.014 |
| NestedFold | safety |     7.300 |      0.091 |
| NestedFold | broken |     3.479 |      0.090 |

これらのモデルは到達できる状態が1〜4個で、rtlcは数µsで終わる。SATは5 bitの整数を8 steps展開する費用を払うので、2〜4桁遅い。[0091](0091-rmodel-language.md)の権限モデル（設定の空間が大きい）とは逆の傾向で、どちらのengineが速いかは問題の形で決まる。[記録](../runs/0095-quint-sat/times.jsonl)。

## 範囲と注意

- 定数のbit幅は変換時に検査するが、状態に依存する算術（例えばCounterで15に1を足す）はSATの中で桁あふれしても検出しない。`forbid overflow`に当たる仕組みはplanの範囲外。
- 集合の`fold`は昇順に畳む。0044の畳み込みはすべて可換なので、順の違いはこのcorpusでは現れない。

## Q2のまとめ

0090（Alloy 6のvarをSATで有界に検査）、0091（構造と振る舞いを1つのモデルで書き、SATと明示状態で同じ意味に検査）と、この0095で、Quintのモデルを含めて、同じモデルを記号的なBMCと明示状態の両方で検査できるようになった。どちらもWasmへビルドできる。
