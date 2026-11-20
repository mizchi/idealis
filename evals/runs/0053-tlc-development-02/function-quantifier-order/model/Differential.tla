---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets
VARIABLE x
Init == x = (\A f \in {[i \in {0, 2} |-> 9], [i \in {0, 3} |-> 0]}: IF f[0] = 0 THEN FALSE ELSE Head(<<>>) = 0)
Next == UNCHANGED x
Prop == x # x
====