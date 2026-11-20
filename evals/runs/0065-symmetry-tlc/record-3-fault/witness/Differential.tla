---- MODULE Differential ----
EXTENDS Naturals, TLC
CONSTANT Nodes
VARIABLE local
Symm == Permutations(Nodes)
Init == local = [n \in Nodes |-> [term |-> 0, ready |-> FALSE]]
Next == \/ (\E n \in Nodes: local' = [local EXCEPT ![n].term = IF @ < 2 THEN @ + 1 ELSE 0])
        \/ (\E n \in Nodes: local' = [local EXCEPT ![n].ready = ~@])
Safe == \A n \in Nodes: local[n].term < 2

CONSTANT __IdealisModelValue0, __IdealisModelValue1, __IdealisModelValue2
VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (local = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2} |-> IF __key0 = __IdealisModelValue0 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue1 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue2 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE TRUE])
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (local' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2} |-> IF __key0 = __IdealisModelValue0 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 1 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue1 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue2 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE TRUE])) \/ (__position = 1 /\ __position' = 2 /\ (local' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2} |-> IF __key0 = __IdealisModelValue0 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 2 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue1 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue2 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE TRUE])) \/ (__position = 2 /\ __position' = 2 /\ (local' = [__key0 \in {__IdealisModelValue0,__IdealisModelValue1,__IdealisModelValue2} |-> IF __key0 = __IdealisModelValue0 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 2 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue1 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE IF __key0 = __IdealisModelValue2 THEN [__key1 \in {"ready","term"} |-> IF __key1 = "ready" THEN FALSE ELSE IF __key1 = "term" THEN 0 ELSE TRUE] ELSE TRUE]))) /\ (Next \/ UNCHANGED <<local>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<local,__position>> /\ WF_<<local,__position>>(__TraceNext)
__Expected == <>(~(Safe))
====