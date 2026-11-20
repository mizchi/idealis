---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {[i \in {0} |-> -2147483647 - 1], [i \in {0} |-> 2147483647]}))
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = <<[__key1 \in {0} |-> IF __key1 = 0 THEN -2147483648 ELSE TRUE],[__key1 \in {0} |-> IF __key1 = 0 THEN 2147483647 ELSE TRUE]>>)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = <<[__key1 \in {0} |-> IF __key1 = 0 THEN -2147483648 ELSE TRUE],[__key1 \in {0} |-> IF __key1 = 0 THEN 2147483647 ELSE TRUE]>>))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====