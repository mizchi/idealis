---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == 1
Init == x = (SelectSeq(<<1>>, Keep))
Next == UNCHANGED x
Safe == x = (TRUE)
====