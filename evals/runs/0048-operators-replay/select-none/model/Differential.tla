---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == FALSE
Init == x = (SelectSeq(<<1,2>>, Keep))
Next == UNCHANGED x
Safe == x = (<<>>)
====