---- MODULE Differential ----
EXTENDS Integers, TLC
VARIABLE x
Before(new, old) == IF <<new, old>> \in {<<2,3>>, <<1,3>>, <<1,2>>} THEN TRUE ELSE 1 \div 0 = 0
Init == x = SortSeq(<<3,2,1>>, Before)
Next == UNCHANGED x
Safe == x = <<1,2,3>>
====