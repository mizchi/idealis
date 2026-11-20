---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x

Init == x = (Assert(TRUE, "ok"))
Next == UNCHANGED x
Safe == x = (TRUE)
====