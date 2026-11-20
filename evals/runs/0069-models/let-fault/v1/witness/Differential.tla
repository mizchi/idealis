---- MODULE Differential ----

VARIABLE x

Init == x = FALSE
Next == x' = (LET left %% right == ~left IN x %% TRUE)
Safe == x = FALSE

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (x = FALSE)
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (x' = TRUE)) \/ (__position = 1 /\ __position' = 1 /\ (x' = TRUE))) /\ (Next \/ UNCHANGED <<x>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<x,__position>> /\ WF_<<x,__position>>(__TraceNext)
__Expected == <>(~(Safe))
====