---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE counters
Symm == Permutations(Nodes)
Init == counters = [node \in Nodes |-> 0]
Next == FALSE /\ counters' = counters
Safe == counters \in [Nodes -> 0..2]
====