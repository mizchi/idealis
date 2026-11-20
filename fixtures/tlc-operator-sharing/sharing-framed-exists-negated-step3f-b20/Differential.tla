---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Id(p) == p
Keep == x' = x
Init == x = 0
Next == Keep
Safe == \E y \in {x}: Id(y # x)
====
