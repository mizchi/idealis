---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Head(<<>>)
Init == x = (ApaFoldSet(Op, 7, {}))
Next == UNCHANGED x
Prop == x # x
====