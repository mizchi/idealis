---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Make(offset) == LET Less(a,b) == a+offset < b+offset IN SortSeq(<<3,1,2>>, Less)
Init == x = (Make(0))
Next == UNCHANGED x
Safe == x = (<<1,2,3>>)
====