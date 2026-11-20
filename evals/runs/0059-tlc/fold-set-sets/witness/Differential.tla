---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Op(a, b) == Append(a, b)
Init == x = (ApaFoldSet(Op, <<>>, {{2}, {1, 0}, {}, {-1}, {0}, {0, 2}}))
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = <<{},{-1},{0},{2},{0,1},{0,2}>>)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = <<{},{-1},{0},{2},{0,1},{0,2}>>))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====