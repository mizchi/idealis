---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Init == x \in {[i \in {0, 2} |-> 9], [i \in {0, 3} |-> 0]}
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = [__key0 \in {0,3} |-> IF __key0 = 0 THEN 0 ELSE IF __key0 = 3 THEN 0 ELSE TRUE])
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = [__key0 \in {0,3} |-> IF __key0 = 0 THEN 0 ELSE IF __key0 = 3 THEN 0 ELSE TRUE]))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====