---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {<<{2, 1}, <<FALSE>>>>, <<{0}, <<TRUE>>>>}))
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = <<<<{0},<<TRUE>>>>,<<{1,2},<<FALSE>>>>>>)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = <<<<{0},<<TRUE>>>>,<<{1,2},<<FALSE>>>>>>))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====