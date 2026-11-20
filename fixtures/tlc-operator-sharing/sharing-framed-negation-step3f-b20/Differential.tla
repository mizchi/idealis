---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Id(p) == p
Step == x' = 1
Init == x = 0
Next == Step
Safe == [](~Id(x # 0))
====
