---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Vars == <<x>>
Init == x = 0
Next == UNCHANGED Vars
Safe == x = 0
====
