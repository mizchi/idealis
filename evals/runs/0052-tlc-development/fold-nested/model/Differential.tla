---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Sub(a, b) == a - b
Init == x = (LET Op(a, b) == a + ApaFoldSeqLeft(Sub, b, <<1>>) IN ApaFoldSet(Op, 0, {3, 2}))
Next == UNCHANGED x
Prop == x # x
====