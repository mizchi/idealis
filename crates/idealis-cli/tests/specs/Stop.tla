---- MODULE Stop ----
EXTENDS Limit, Naturals
VARIABLE x
Init == x = Max
Next == x < Max /\ x' = x + 1
====
