---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step(a) == x' = 1 /\ a' = 1
Init == x = 0
Next == \E p \in {0}: Step(x + p)
Safe == x = 0
====
