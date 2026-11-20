---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(a,b) == TRUE
Init == x = (SelectSeq(<<1>>, Keep))
Next == UNCHANGED x
Safe == x = (TRUE)
====