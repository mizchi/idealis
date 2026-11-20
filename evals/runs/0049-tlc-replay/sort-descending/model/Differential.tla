---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == a > b
Init == x = (SortSeq(<<3,1,2,1>>, Less))
Next == UNCHANGED x
Safe == x = (<<3,2,1,1>>)
====