---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE counters
Symm == Permutations(Nodes)
Init == counters = [node \in Nodes |-> 0]
Next == \E node \in Nodes: counters' = [counters EXCEPT ![node] = IF @ < 2 THEN @ + 1 ELSE 0]
Safe == \A node \in Nodes: counters[node] < 2

CONSTANT __IdealisModelValue0, __IdealisModelValue1, __IdealisModelValue2, __IdealisModelValue3
VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (counters = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2,__IdealisModelValue3} |-> IF __key0 = __IdealisModelValue0 THEN 0 ELSE IF __key0 = __IdealisModelValue1 THEN 0 ELSE IF __key0 = __IdealisModelValue2 THEN 0 ELSE IF __key0 = __IdealisModelValue3 THEN 0 ELSE TRUE])
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (counters' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2,__IdealisModelValue3} |-> IF __key0 = __IdealisModelValue0 THEN 1 ELSE IF __key0 = __IdealisModelValue1 THEN 0 ELSE IF __key0 = __IdealisModelValue2 THEN 0 ELSE IF __key0 = __IdealisModelValue3 THEN 0 ELSE TRUE])) \/ (__position = 1 /\ __position' = 2 /\ (counters' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2,__IdealisModelValue3} |-> IF __key0 = __IdealisModelValue0 THEN 2 ELSE IF __key0 = __IdealisModelValue1 THEN 0 ELSE IF __key0 = __IdealisModelValue2 THEN 0 ELSE IF __key0 = __IdealisModelValue3 THEN 0 ELSE TRUE])) \/ (__position = 2 /\ __position' = 2 /\ (counters' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2,__IdealisModelValue3} |-> IF __key0 = __IdealisModelValue0 THEN 2 ELSE IF __key0 = __IdealisModelValue1 THEN 0 ELSE IF __key0 = __IdealisModelValue2 THEN 0 ELSE IF __key0 = __IdealisModelValue3 THEN 0 ELSE TRUE]))) /\ (Next \/ UNCHANGED <<counters>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<counters,__position>> /\ WF_<<counters,__position>>(__TraceNext)
__Expected == <>(~(Safe))
====