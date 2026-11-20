---- MODULE Differential ----
EXTENDS Integers
VARIABLES x, y
Step(p) == /\ x' = p
           /\ y' = 3 - p
Init == /\ x = 0
        /\ y = 0
Next == \E p \in {1, 2}: Step(p)
Safe == /\ x \in 0..2
        /\ y \in 0..2
====
