---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Init == x \in {[i \in {0, 2} |-> 9], [i \in {0, 3} |-> 0]}
Next == UNCHANGED x
Prop == x # x
====