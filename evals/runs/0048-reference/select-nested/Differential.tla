---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Inner(v) == v > 1
Keep(v) == Len(SelectSeq(<<v>>, Inner)) = 1
Init == x = (SelectSeq(<<1,2>>, Keep))
Next == UNCHANGED x
Safe == x = (<<2>>)
====