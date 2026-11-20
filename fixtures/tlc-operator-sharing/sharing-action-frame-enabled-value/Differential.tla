---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step(a) == x' = a /\ x' = 1
Init == x = 0
Next == UNCHANGED x
Safe == \A p \in {1, 2}: (ENABLED Step(p)) <=> (p = 1)
====
