---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Advance == x < 3 /\ x' = x + 1
Stay == UNCHANGED x
Unused == FALSE /\ x' = 0
Next == Advance \/ Stay \/ Unused
Live == <> (x = 9)

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = 0)
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (x' = 0))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == ~(Live)
====