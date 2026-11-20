---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {{2}, {1, 0}, {}, {-1}, {0}, {0, 2}}))
Next == UNCHANGED x
Prop == x # x
====