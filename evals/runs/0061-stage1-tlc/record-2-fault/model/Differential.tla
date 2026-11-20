---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE local
Symm == Permutations(Nodes)
Init == local = [n \in Nodes |-> [term |-> 0, ready |-> FALSE]]
Next == \/ (\E n \in Nodes: local' = [local EXCEPT ![n].term = IF @ < 2 THEN @ + 1 ELSE 0])
        \/ (\E n \in Nodes: local' = [local EXCEPT ![n].ready = ~@])
Safe == \A n \in Nodes: local[n].term < 2
====