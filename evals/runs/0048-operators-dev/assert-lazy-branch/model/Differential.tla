---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x

Init == x = (IF TRUE THEN 7 ELSE Assert(FALSE, "unused"))
Next == UNCHANGED x
Safe == x = (7)
====