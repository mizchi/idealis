---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == v > 1
Init == x = (SelectSeq([i \in 1..3 |-> i], Keep))
Next == UNCHANGED x
Safe == x = (<<2,3>>)
====