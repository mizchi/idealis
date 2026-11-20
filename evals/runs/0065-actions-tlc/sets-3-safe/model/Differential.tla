---- MODULE Differential ----
EXTENDS TLC
CONSTANT Nodes
VARIABLES first, second
Symm == Permutations(Nodes)
Init == /\ first = {} /\ second = {}
Next == \/ (\E n \in Nodes: /\ first' = (IF n \in first THEN first \ {n} ELSE first \cup {n}) /\ UNCHANGED second)
        \/ (\E n \in Nodes: /\ second' = (IF n \in second THEN second \ {n} ELSE second \cup {n}) /\ UNCHANGED first)
Safe == /\ first \subseteq Nodes /\ second \subseteq Nodes
====