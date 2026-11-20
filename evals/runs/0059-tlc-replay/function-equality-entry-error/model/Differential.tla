---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
F == [i \in {0, 2} |-> TRUE]
G == [i \in {0, 3} |-> 0]
Init == x = (F = G)
Next == UNCHANGED x
Prop == x # x
====