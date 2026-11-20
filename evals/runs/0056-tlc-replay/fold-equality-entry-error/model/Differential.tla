---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
F == [i \in {0, 2} |-> TRUE]
G == [i \in {0, 3} |-> 0]
Op(a, b) == b = G
Init == x = (ApaFoldSet(Op, FALSE, {F}))
Next == UNCHANGED x
Prop == x # x
====