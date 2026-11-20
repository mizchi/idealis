---- MODULE Missing ----
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
A == x < 2 /\ x' = x + 1
Next == A \/ (x = 2 /\ UNCHANGED <<x, y>>)
====
