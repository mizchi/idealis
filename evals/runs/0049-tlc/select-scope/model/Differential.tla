---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Make(limit) == LET Keep(v) == v > limit IN SelectSeq(<<1,2,3>>, Keep)
Init == x = (Make(1))
Next == UNCHANGED x
Safe == x = (<<2,3>>)
====