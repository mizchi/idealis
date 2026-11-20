---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
F(p) == ENABLED (CASE UNCHANGED p -> UNCHANGED x [] OTHER -> UNCHANGED x)
Init == x = 0
Next == UNCHANGED x
Safe == F(0)
====
