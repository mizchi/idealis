---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == TRUE
Init == x = (SortSeq("abc", Less))
Next == UNCHANGED x
Safe == x = (TRUE)
====