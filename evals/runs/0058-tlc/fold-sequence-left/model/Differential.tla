---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == a * 10 + b
Init == x = (ApaFoldSeqLeft(Op, 20, <<3, 1, 4>>))
Next == UNCHANGED x
Prop == x # x
====