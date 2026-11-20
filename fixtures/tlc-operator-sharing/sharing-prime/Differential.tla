---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Id(p) == p
Act(p) == x' = p
Prime(p) == p'
Init == x = 0
Next == Act(Id(1-x))
Safe == x \in {0,1}
P == [][Prime(Id(x)) = Id(1-x)]_x
====
