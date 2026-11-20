---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
V0 == x
V1 == <<V0, V0>>
V2 == <<V1, V1>>
V3 == <<V2, V2>>
V4 == <<V3, V3>>
V5 == <<V4, V4>>
V6 == <<V5, V5>>
Init == x = 0
Next == x' = x
Spec == Init /\ [][Next]_V6
Safe == x = 0
====
