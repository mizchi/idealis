---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Wrap(p) == \A c \in {9}: p
Add(a,b) == a+b
Safe == Wrap(\A z \in {0}: z = 0)
====
