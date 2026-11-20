---- MODULE Differential ----
VARIABLE x
Temporal(p) == [](p)
P == Temporal((\A b \in {0}: b = 0))
Init == x = 0
Next == UNCHANGED x
Safe == x = 0
====
