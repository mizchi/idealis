---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Step == x' = 1
Con == x = 0
Init == x = 0
Next == Step
Safe == [](x = 0 /\ TRUE)
====
