---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Head(<<>>)
Init == x = (ApaFoldSeqLeft(Op, 9, <<>>))
Next == UNCHANGED x
Prop == x # x
====