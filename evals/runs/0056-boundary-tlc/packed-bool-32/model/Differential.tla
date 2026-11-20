---- MODULE Differential ----
EXTENDS Naturals
CONSTANT N
VARIABLE f
Init == f = [i \in 1..N |-> FALSE]
Next == f' = [f EXCEPT ![1] = ~@]
Safe == f \in [1..N -> BOOLEAN]
====