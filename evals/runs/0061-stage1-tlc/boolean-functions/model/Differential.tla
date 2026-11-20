---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE counters
Symm == Permutations(Nodes)
Init == counters = [node \in Nodes |-> FALSE]
Next == \E node \in Nodes: counters' = [counters EXCEPT ![node] = ~@]
Safe == counters \in [Nodes -> BOOLEAN]
====