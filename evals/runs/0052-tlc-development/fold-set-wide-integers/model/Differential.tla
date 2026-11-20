---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {-2147483647 - 1, 2147483647}))
Next == UNCHANGED x
Prop == x # x
====