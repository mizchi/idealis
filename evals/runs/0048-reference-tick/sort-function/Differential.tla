---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == a < b
Init == x = (SortSeq([i \in 1..3 |-> 4-i], Less))
Next == UNCHANGED x
Safe == x = (<<1,2,3>>)
====