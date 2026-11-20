---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == TRUE
Init == x = (SelectSeq("abc", Keep))
Next == UNCHANGED x
Safe == x = (TRUE)
====