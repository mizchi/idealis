---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x

Init == x = (LET offset == 10 Op(a, b) == Append(a, b + offset) IN ApaFoldSet(Op, <<>>, {2, 1}))
Next == UNCHANGED x
Prop == x # x
====