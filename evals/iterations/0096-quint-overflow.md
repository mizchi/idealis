# 0096: SATの経路で整数の桁あふれを検出し、判定を未確定にする

[plan](../plans/0096-quint-overflow.md)のとおり、`rquint::to_alloy`が状態に依存する足し算・引き算・符号反転ごとに桁あふれしない条件を作り、`c_overflow`という専用のcommandとして出すようにした。`c_overflow`に反例があれば、他のcommandの判定は未確定として扱う。

- **Overflow.qnt**（0から3ずつ増える）: 8 stepsで21に達し、5 bit（-16〜15）を超える。`c_overflow`は反例を持ち、桁あふれを検出した。guardを見ずに`c_nonnegative`だけを解くと、桁が回った-11による偽の反例が出る（ralloyでもAlloy 6.2.0でも同じ）。rtlc（深さ8で打ち切り）は`nonnegative`に違反を見つけず、`small`には見つけた。
- **0095の10件**: `c_overflow`はどれも反例を持たず、判定は0095と同じ。Alloy 6.2.0の答えは、新しいcommandを含む出力に対して記録し直した（判定は変わらない）。
- **掛け算**: 状態に依存する`imul`は「multiplication of state-dependent integers for SAT」として拒否する。
- **故障**: 足し算の条件を落とすと、Overflowの`c_overflow`が反例を持たなくなり、テストが検出した。

## 注意

- 条件は、`if`の枝や`match`の場合分けの中の算術にも無条件にかかる。使われない枝で桁あふれする場合も未確定になる。偽の判定は出さないが、判定を出せる範囲は狭くなる側に倒している。
- nondetの値に依存する算術の条件は、その値の範囲について`all`で閉じる。
