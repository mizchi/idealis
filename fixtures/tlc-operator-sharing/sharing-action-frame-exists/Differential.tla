---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step(p) == x' = p
Init == x = 0
Next == \E p \in {1, 2}: Step(p)
Safe == x \in 0..2
====
