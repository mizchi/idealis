---- MODULE Operators ----
EXTENDS Integers, Sequences, TLC
CONSTANT Limit
VARIABLE state
Keep(v) == v > Limit
Before(left,right) == left < right
Init == state = SortSeq(SelectSeq(<<3,1,2,2>>, Keep), Before)
Next == UNCHANGED state
Safe == Assert(state = <<2,2,3>>, "instance")
====