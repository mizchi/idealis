---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step(a) == /\ [<<4>> EXCEPT ![1] = a][1] = 9
           /\ x' = x
Init == x = 0
Next == UNCHANGED x
Safe == [<<9>> EXCEPT ![1] = ENABLED Step(@)][1]
====
