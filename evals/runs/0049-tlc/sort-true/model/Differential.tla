---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == TRUE
Init == x = (SortSeq(<<3,1,2>>, Less))
Next == UNCHANGED x
Safe == x = (<<2,1,3>>)
====