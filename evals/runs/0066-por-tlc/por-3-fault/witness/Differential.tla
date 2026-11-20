---- MODULE Differential ----
EXTENDS Naturals
VARIABLES visible,flag0,flag1,flag2
Init == visible = 0 /\ flag0 = FALSE /\ flag1 = FALSE /\ flag2 = FALSE
Next == (/\ ~flag0 /\ flag0' = TRUE /\ UNCHANGED <<visible,flag1,flag2>>) \/ (/\ ~flag1 /\ flag1' = TRUE /\ UNCHANGED <<visible,flag0,flag2>>) \/ (/\ ~flag2 /\ flag2' = TRUE /\ UNCHANGED <<visible,flag0,flag1>>) \/ (visible' = (IF visible < 3 THEN visible + 1 ELSE 0) /\ UNCHANGED <<flag0,flag1,flag2>>)
Safe == visible < 3

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (visible = 0 /\ flag0 = FALSE /\ flag1 = FALSE /\ flag2 = FALSE)
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (visible' = 0 /\ flag0' = TRUE /\ flag1' = FALSE /\ flag2' = FALSE)) \/ (__position = 1 /\ __position' = 2 /\ (visible' = 0 /\ flag0' = TRUE /\ flag1' = TRUE /\ flag2' = FALSE)) \/ (__position = 2 /\ __position' = 3 /\ (visible' = 0 /\ flag0' = TRUE /\ flag1' = TRUE /\ flag2' = TRUE)) \/ (__position = 3 /\ __position' = 4 /\ (visible' = 1 /\ flag0' = TRUE /\ flag1' = TRUE /\ flag2' = TRUE)) \/ (__position = 4 /\ __position' = 5 /\ (visible' = 2 /\ flag0' = TRUE /\ flag1' = TRUE /\ flag2' = TRUE)) \/ (__position = 5 /\ __position' = 6 /\ (visible' = 3 /\ flag0' = TRUE /\ flag1' = TRUE /\ flag2' = TRUE)) \/ (__position = 6 /\ __position' = 6 /\ (visible' = 3 /\ flag0' = TRUE /\ flag1' = TRUE /\ flag2' = TRUE))) /\ (Next \/ UNCHANGED <<visible,flag0,flag1,flag2>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<visible,flag0,flag1,flag2,__position>> /\ WF_<<visible,flag0,flag1,flag2,__position>>(__TraceNext)
__Expected == <>(~(Safe))
====