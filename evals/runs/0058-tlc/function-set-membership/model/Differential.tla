---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets
VARIABLE x
Init == x = ([i \in {0, 3} |-> 0] \in {[i \in {0, 2} |-> 9], [i \in {0, 3} |-> 0]})
Next == UNCHANGED x
Prop == x # x
====