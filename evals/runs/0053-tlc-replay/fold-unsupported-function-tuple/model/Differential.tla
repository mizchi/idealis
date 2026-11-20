---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {<<[i \in {0} |-> 0]>>}))
Next == UNCHANGED x
Prop == x # x
====