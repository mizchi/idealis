---- MODULE Differential ----
EXTENDS Integers, Sequences, FiniteSets, Apalache
VARIABLE x
Sub(a, b) == a - b
Init == x = (LET Op(a, b) == a + ApaFoldSeqLeft(Sub, b, <<1>>) IN ApaFoldSet(Op, 0, {3, 2}))
Next == UNCHANGED x
Prop == x # x

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = 3)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = 3))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Prop))
====