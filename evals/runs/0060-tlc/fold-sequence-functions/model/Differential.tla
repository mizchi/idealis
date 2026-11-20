---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSeqLeft(Op, <<>>, <<[i \in {0} |-> 1], <<2>>, {FALSE}, TRUE, 3>>))
Next == UNCHANGED x
Prop == x # x
====