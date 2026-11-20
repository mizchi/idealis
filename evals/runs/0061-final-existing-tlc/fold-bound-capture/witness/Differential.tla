---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x

Init == x = ([n \in {2, 3} |-> LET Op(a, b) == a + b + n IN ApaFoldSeqLeft(Op, 0, <<1>>)])
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = [__key0 \in {2,3} |-> IF __key0 = 2 THEN 3 ELSE IF __key0 = 3 THEN 4 ELSE TRUE])
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = [__key0 \in {2,3} |-> IF __key0 = 2 THEN 3 ELSE IF __key0 = 3 THEN 4 ELSE TRUE]))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====