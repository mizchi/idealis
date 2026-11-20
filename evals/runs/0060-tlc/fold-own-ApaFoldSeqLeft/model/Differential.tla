---- MODULE Differential ----
VARIABLE x
ApaFoldSeqLeft(a, b, c) == b
Init == x = ApaFoldSeqLeft(0, 7, {})
Next == UNCHANGED x
Prop == x # x
====