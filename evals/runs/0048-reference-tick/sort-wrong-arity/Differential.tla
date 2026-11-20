---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a) == TRUE
Init == x = (SortSeq(<<1>>, Less))
Next == UNCHANGED x
Safe == x = (TRUE)
====