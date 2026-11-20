---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == Act(Id(1-x))
Id(p) == p
Act(p) == x' = p
Prime(p) == p'
Safe == x \in {0,1} /\ ENABLED (Act(Id(1-x)) /\ x' # x) /\ ~ENABLED (Act(Id(x)) /\ x' # x)
====
