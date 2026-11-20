---- MODULE Differential ----
VARIABLE x
P == []TRUE
Init == x = 0
Next == UNCHANGED x
Safe == x = 0
====
