---- MODULE Differential ----
EXTENDS TLC
CONSTANT Nodes
VARIABLES first, second
Symm == Permutations(Nodes)
Init == /\ first = {} /\ second = {}
Next == \/ (\E n \in Nodes: /\ first' = (IF n \in first THEN first \ {n} ELSE first \cup {n}) /\ UNCHANGED second)
        \/ (\E n \in Nodes: /\ second' = (IF n \in second THEN second \ {n} ELSE second \cup {n}) /\ UNCHANGED first)
Safe == /\ first = {} /\ second \subseteq Nodes

CONSTANT __IdealisModelValue0
VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (first = {} /\ second = {})
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (first' = {__IdealisModelValue0} /\ second' = {})) \/ (__position = 1 /\ __position' = 1 /\ (first' = {__IdealisModelValue0} /\ second' = {}))) /\ (Next \/ UNCHANGED <<first,second>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<first,second,__position>> /\ WF_<<first,second,__position>>(__TraceNext)
__Expected == <>(~(Safe))
====