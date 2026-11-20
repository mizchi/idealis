---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == 1 \div 0 = 0
Init == x = (SelectSeq(<<>>, Keep))
Next == UNCHANGED x
Safe == x = (<<>>)
====