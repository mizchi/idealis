---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step(a) == x' = a
Init == x = 0
Next == UNCHANGED x
Safe == \A p \in {1, 2}: ENABLED Step(p)
====
