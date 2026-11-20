---- MODULE Differential ----
EXTENDS Naturals
VARIABLES visible,flag0
Init == visible = 0 /\ flag0 = FALSE
Next == (/\ ~flag0 /\ flag0' = TRUE /\ UNCHANGED <<visible>>) \/ (~flag0 /\ visible' = (IF visible < 3 THEN visible + 1 ELSE 0) /\ UNCHANGED <<flag0>>)
Safe == visible <= 3
__HasNext == ENABLED Next

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (visible = 0 /\ flag0 = FALSE)
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (visible' = 0 /\ flag0' = TRUE)) \/ (__position = 1 /\ __position' = 1 /\ (visible' = 0 /\ flag0' = TRUE))) /\ (Next \/ UNCHANGED <<visible,flag0>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<visible,flag0,__position>> /\ WF_<<visible,flag0,__position>>(__TraceNext)
__Expected == <>(~(__HasNext))
====