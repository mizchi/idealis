---- MODULE Differential ----
EXTENDS TLC
CONSTANT Nodes
VARIABLES first, second
Symm == Permutations(Nodes)
Init == /\ first = [n \in Nodes |-> FALSE] /\ second = [n \in Nodes |-> FALSE]
Next == \/ (\E n \in Nodes: /\ first' = [first EXCEPT ![n] = ~@] /\ UNCHANGED second)
        \/ (\E n \in Nodes: /\ second' = [second EXCEPT ![n] = ~@] /\ UNCHANGED first)
Safe == /\ first \in [Nodes -> BOOLEAN] /\ second \in [Nodes -> BOOLEAN]
====