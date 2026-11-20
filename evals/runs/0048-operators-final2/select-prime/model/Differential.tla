---- MODULE Differential ----
EXTENDS Integers, Sequences, TLC
VARIABLE x
Keep(v) == v > x
Init == x = 2
Next == x' = 1 /\ SelectSeq(<<1,2>>, Keep)' = <<2>>
Safe == x \in {1,2}
====