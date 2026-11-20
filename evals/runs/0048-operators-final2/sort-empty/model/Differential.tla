---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Less(a,b) == 1 \div 0 = 0
Init == x = (SortSeq(<<>>, Less))
Next == UNCHANGED x
Safe == x = (<<>>)
====