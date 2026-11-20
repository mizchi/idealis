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
V7 == <<V6, V6>>
V8 == <<V7, V7>>
V9 == <<V8, V8>>
V10 == <<V9, V9>>
V11 == <<V10, V10>>
V12 == <<V11, V11>>
V13 == <<V12, V12>>
V14 == <<V13, V13>>
V15 == <<V14, V14>>
V16 == <<V15, V15>>
Init == x = 0
Next == x' = x
Spec == Init /\ [][Next]_V16
Safe == x = 0
====
