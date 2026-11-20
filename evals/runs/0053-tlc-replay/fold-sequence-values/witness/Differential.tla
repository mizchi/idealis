---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSeqLeft(Op, <<>>, <<"z", [a |-> 1], "A">>))
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = <<"z",[__key1 \in {"a"} |-> IF __key1 = "a" THEN 1 ELSE TRUE],"A">>)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = <<"z",[__key1 \in {"a"} |-> IF __key1 = "a" THEN 1 ELSE TRUE],"A">>))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====