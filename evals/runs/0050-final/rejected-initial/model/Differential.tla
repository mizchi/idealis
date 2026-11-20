---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x \in 0..1 /\ x > 0
Next == UNCHANGED x
====