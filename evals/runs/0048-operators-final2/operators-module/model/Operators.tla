---- MODULE Operators ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == v > 1
Before(left,right) == left < right
Init == x = SortSeq(SelectSeq(<<3,1,2,2>>, Keep), Before)
Next == UNCHANGED x
Safe == Assert(x = <<2,2,3>>, "module")
====