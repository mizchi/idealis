---- MODULE Counter ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1
Grow == x' = x + 1
Empty == x \in {}
Safe == x < 2
Broken == x = 0
====
