---- MODULE Differential ----
VARIABLE x
P == [](\A b \in {0}: b = 0)
Init == x = 0
Next == UNCHANGED x
Safe == x = 0
====
