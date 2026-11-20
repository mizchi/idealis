---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Can(p) == (\A z \in {9}: p) /\ ENABLED (UNCHANGED x)
Init == x = 0
Next == UNCHANGED x
Safe == Can(TRUE)
====
