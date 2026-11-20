---- MODULE Differential ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Safe == LET Assert(condition, message) == condition IN Assert(TRUE, 1 \div 0)
====