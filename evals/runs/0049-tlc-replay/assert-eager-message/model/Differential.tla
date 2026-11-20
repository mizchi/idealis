---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x

Init == x = (Assert(TRUE, 1 \div 0))
Next == UNCHANGED x
Safe == x = (TRUE)
====