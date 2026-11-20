---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE counters
Symm == Permutations(Nodes)
Init == counters = [node \in Nodes |-> 0]
Next == \E node \in Nodes: counters' = [counters EXCEPT ![node] = IF @ < 2 THEN @ + 1 ELSE 0]
Safe == counters \in [Nodes -> 0..2]
====