---- MODULE Differential ----
EXTENDS Naturals, Sequences, Apalache
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Wrap(p) == \A c \in {9}: p
Add(a,b) == a+b
Safe == Wrap(ApaFoldSeqLeft(Add, 0, <<0,1>>) = 1)
====
