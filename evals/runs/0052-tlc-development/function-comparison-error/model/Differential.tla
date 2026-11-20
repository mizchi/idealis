---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets
VARIABLE x
Init == x = (Cardinality({[i \in {0, 2} |-> TRUE], [i \in {0, 3} |-> 0]}))
Next == UNCHANGED x
Prop == x # x
====