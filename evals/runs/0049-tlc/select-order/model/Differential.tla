---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == v > 1
Init == x = (SelectSeq(<<3,1,2,3>>, Keep))
Next == UNCHANGED x
Safe == x = (<<3,2,3>>)
====