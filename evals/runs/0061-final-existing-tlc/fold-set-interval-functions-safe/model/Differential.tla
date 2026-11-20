---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {[i \in {-2147483647} |-> 0], [i \in 0..0 |-> 1]}))
Next == UNCHANGED x
Prop == x # x
====