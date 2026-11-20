---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
Less(a,b) == TRUE
Init == x = (SortSeq(<<1>>, Less))
Next == UNCHANGED x
Safe == x = (TRUE)
====