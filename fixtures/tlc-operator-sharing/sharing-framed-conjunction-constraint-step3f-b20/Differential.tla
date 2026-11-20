---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Id(p) == p
Step == x' = 1
Con == x = 0
Init == x = 0
Next == Step
Safe == [](Id(x = 0) /\ TRUE)
====
