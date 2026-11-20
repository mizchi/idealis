---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = 0 \/ x = 0 \/ x = 1
Next == UNCHANGED x
====