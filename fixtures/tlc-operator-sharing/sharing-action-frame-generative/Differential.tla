---- MODULE Differential ----
VARIABLE x
Act(p) == (\A z \in {9}: p) /\ UNCHANGED x
Init == x = 0
Next == Act((\A b \in {0}: b = 0))
Safe == x = 0
====
