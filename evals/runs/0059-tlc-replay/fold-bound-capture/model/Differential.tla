---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x

Init == x = ([n \in {2, 3} |-> LET Op(a, b) == a + b + n IN ApaFoldSeqLeft(Op, 0, <<1>>)])
Next == UNCHANGED x
Prop == x # x
====