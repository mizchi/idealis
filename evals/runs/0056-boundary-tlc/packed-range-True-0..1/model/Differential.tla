---- MODULE Differential ----
EXTENDS Naturals
VARIABLE f
Init == f = [i \in 1..2 |-> FALSE]
Next == f' = [f EXCEPT ![1] = ~@]
Safe == f \in [1..2 -> 0..1]
====