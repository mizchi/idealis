---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == TRUE
Init == x = (SelectSeq(<<1,2>>, Keep))
Next == UNCHANGED x
Safe == x = (<<1,2>>)
====