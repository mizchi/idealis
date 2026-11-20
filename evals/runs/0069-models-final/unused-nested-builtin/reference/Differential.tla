---- MODULE Differential ----

VARIABLE x
Unused == LET left /\ right == FALSE IN TRUE
Init == x = 0
Next == UNCHANGED x
====
