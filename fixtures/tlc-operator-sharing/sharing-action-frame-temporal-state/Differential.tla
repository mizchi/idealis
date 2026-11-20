---- MODULE Differential ----
VARIABLE x
Temporal(p) == [](p)
P == Temporal((\A b \in {0}: b = x))
Init == x = 0
Next == UNCHANGED x
Safe == x = 0
====
