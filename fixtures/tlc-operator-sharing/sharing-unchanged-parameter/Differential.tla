---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Vars == <<x>>
Keep(p) == UNCHANGED p
Init == x = 0
Next == Keep(Vars)
Safe == x = 0
====
