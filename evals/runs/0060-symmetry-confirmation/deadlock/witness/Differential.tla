---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE counters
Symm == Permutations(Nodes)
Init == counters = [node \in Nodes |-> 0]
Next == FALSE /\ counters' = counters
Safe == counters \in [Nodes -> 0..2]
__HasNext == ENABLED Next

CONSTANT __IdealisModelValue0, __IdealisModelValue1, __IdealisModelValue2
VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (counters = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2} |-> IF __key0 = __IdealisModelValue0 THEN 0 ELSE IF __key0 = __IdealisModelValue1 THEN 0 ELSE IF __key0 = __IdealisModelValue2 THEN 0 ELSE TRUE])
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (counters' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2} |-> IF __key0 = __IdealisModelValue0 THEN 0 ELSE IF __key0 = __IdealisModelValue1 THEN 0 ELSE IF __key0 = __IdealisModelValue2 THEN 0 ELSE TRUE]))) /\ (Next \/ UNCHANGED <<counters>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<counters,__position>> /\ WF_<<counters,__position>>(__TraceNext)
__Expected == <>(~(__HasNext))
====