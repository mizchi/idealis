---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Inner == <<x>>
Vars == Inner
Init == x = 0
Next == UNCHANGED Vars
Safe == x = 0
====
