---- MODULE Differential ----
EXTENDS Naturals
VARIABLE f
Init == f = [i \in 1..2 |-> 0]
Next == f' = [f EXCEPT ![1] = 1-@]
Safe == f \in [1..2 -> {0}]

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (f = <<0,0>>)
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (f' = <<1,0>>)) \/ (__position = 1 /\ __position' = 1 /\ (f' = <<1,0>>))) /\ (Next \/ UNCHANGED <<f>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<f,__position>> /\ WF_<<f,__position>>(__TraceNext)
__Expected == <>(~(Safe))
====