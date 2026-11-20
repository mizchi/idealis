---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == a.rank < b.rank
Init == x = (SortSeq(<<[rank |-> 0, id |-> 1],[rank |-> 0, id |-> 2]>>, Less))
Next == UNCHANGED x
Safe == x = (<<[rank |-> 0, id |-> 1],[rank |-> 0, id |-> 2]>>)
====