---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == 1
Init == x = (SortSeq(<<1,2>>, Less))
Next == UNCHANGED x
Safe == x = (TRUE)
====