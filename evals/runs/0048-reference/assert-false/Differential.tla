---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x

Init == x = (Assert(FALSE, "expected failure"))
Next == UNCHANGED x
Safe == x = (TRUE)
====