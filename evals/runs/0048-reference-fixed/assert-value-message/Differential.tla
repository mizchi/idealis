---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x

Init == x = (Assert(TRUE, [tag |-> 2]))
Next == UNCHANGED x
Safe == x = (TRUE)
====