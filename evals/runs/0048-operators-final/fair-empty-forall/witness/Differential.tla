---- MODULE Differential ----
EXTENDS Integers
VARIABLE done
Init == done = {}
Step(p) == p \notin done /\ done' = done \cup {p}
Next == \E p \in {1,2}: Step(p)
Fair(processes) == \A p \in processes: WF_done(Step(p))
Spec == Init /\ [][Next]_done /\ (\A p \in {}: WF_done(Step(p)))
Live == <> (done = {1,2})

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (done = {})
__TraceNext == ((__position = 0 /\ __position' = 0 /\ (done' = {}))) /\ (Next \/ UNCHANGED <<done>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<done,__position>> /\ WF_<<done,__position>>(__TraceNext)
__Expected == (Spec) /\ ~(Live)
====