---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x

Init == x = (Assert(1, "not Boolean"))
Next == UNCHANGED x
Safe == x = (TRUE)
====