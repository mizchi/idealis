---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
D(p) == CHOOSE n \in p: TRUE
Init == x = 0
Next == UNCHANGED x
Safe == \A n \in {9}: D({n}) = x
====
