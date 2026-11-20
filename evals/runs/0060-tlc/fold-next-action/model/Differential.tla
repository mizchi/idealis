---- MODULE Differential ----
EXTENDS Integers, Apalache
VARIABLE x
Op(a, b) == a - b
Init == x = 5
Next == IF x = 5 THEN x' = ApaFoldSeqLeft(Op, x, <<2, 1>>) ELSE UNCHANGED x
Safe == x > 0
====