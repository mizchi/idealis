---- MODULE Differential ----

VARIABLE x
Unused(_ /\ _) == TRUE
Init == x = 0
Next == UNCHANGED x
====
