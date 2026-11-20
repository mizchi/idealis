---- MODULE FunctionCube ----
EXTENDS Naturals
CONSTANT N
VARIABLE nodes
Init == nodes = [node \in 1..N |-> 0]
Next == \E node \in 1..N: nodes' = [nodes EXCEPT ![node] = 1-@]
Safe == nodes \in [1..N -> 0..1]
====