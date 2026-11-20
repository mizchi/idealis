---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
Keep(v) == v > 1
Filter(op(_)) == SelectSeq(<<1,2>>, op)
Init == x = Filter(Keep)
Next == UNCHANGED x
Safe == x = <<2>>
====