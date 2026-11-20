---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Types(p) == IF ENABLED (UNCHANGED x) THEN {0} ELSE {1}
N == Nat
Init == x = 0
Next == UNCHANGED x
Safe == x \in Types(N)
====
