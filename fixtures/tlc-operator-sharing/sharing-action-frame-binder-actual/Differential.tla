---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step(a) == \E q \in {1}: x' = IF a THEN q ELSE 0
Init == x = 0
Next == Step((\A q \in {0}: q = 0))
Safe == x \in 0..1
====
