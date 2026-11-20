---- MODULE Differential ----
VARIABLE x
ApaFoldSet(a, b, c) == b
Init == x = ApaFoldSet(0, 7, {})
Next == UNCHANGED x
Prop == x # x
====