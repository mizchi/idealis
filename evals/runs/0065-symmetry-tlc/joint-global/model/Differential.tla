---- MODULE Differential ----
EXTENDS TLC
CONSTANT Nodes
VARIABLES first, second, phase
Symm == Permutations(Nodes)
Init == /\ first = [n \in Nodes |-> FALSE] /\ second = [n \in Nodes |-> FALSE] /\ phase = FALSE
Next == \/ (\E n \in Nodes: /\ first' = [first EXCEPT ![n] = ~@] /\ UNCHANGED <<second, phase>>)
        \/ (\E n \in Nodes: /\ second' = [second EXCEPT ![n] = ~@] /\ UNCHANGED <<first, phase>>)
        \/ (/\ phase' = ~phase /\ UNCHANGED <<first, second>>)
Safe == /\ first \in [Nodes -> BOOLEAN] /\ second \in [Nodes -> BOOLEAN]
====