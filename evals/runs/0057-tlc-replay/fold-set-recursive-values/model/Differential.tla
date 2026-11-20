---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {<<{2, 1}, <<FALSE>>>>, <<{0}, <<TRUE>>>>}))
Next == UNCHANGED x
Prop == x # x
====