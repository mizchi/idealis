---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSeqLeft(Op, <<>>, <<"z", [a |-> 1], "A">>))
Next == UNCHANGED x
Prop == x # x
====