---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {[i \in {0} |-> 0]}))
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = <<[__key1 \in {0} |-> IF __key1 = 0 THEN 0 ELSE TRUE]>>)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = <<[__key1 \in {0} |-> IF __key1 = 0 THEN 0 ELSE TRUE]>>))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====