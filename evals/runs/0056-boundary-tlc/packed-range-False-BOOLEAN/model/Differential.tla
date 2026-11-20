---- MODULE Differential ----
EXTENDS Naturals
VARIABLE f
Init == f = [i \in 1..2 |-> 0]
Next == f' = [f EXCEPT ![1] = 1-@]
Safe == f \in [1..2 -> BOOLEAN]
====