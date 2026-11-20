---- MODULE Differential ----
EXTENDS Naturals
CONSTANT N
VARIABLE f
Init == f = [i \in 1..N |-> 0]
Next == f' = [f EXCEPT ![1] = 1-@]
Safe == f \in [1..N -> 0..1]
====