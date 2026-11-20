---- MODULE Differential ----
EXTENDS Integers
VARIABLE x

Init == x = (Assert(TRUE, "ok"))
Next == UNCHANGED x
Safe == x = (TRUE)
====