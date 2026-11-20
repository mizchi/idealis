---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == v = 2
Init == x = (SelectSeq(<<2,2,1,2>>, Keep))
Next == UNCHANGED x
Safe == x = (<<2,2,2>>)
====