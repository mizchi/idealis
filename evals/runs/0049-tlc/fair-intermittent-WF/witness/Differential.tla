---- MODULE Differential ----
EXTENDS Integers
VARIABLES turn, done
vars == <<turn,done>>
Init == turn = 0 /\ done = {}
Tick == turn' = 1-turn /\ UNCHANGED done
Step(p) == turn = 1 /\ p \notin done /\ turn' = 0 /\ done' = done \cup {p}
Next == Tick \/ (\E p \in {1,2}: Step(p))
FairTick == WF_vars(Tick)
Spec == Init /\ [][Next]_vars /\ FairTick /\ (\A p \in {1,2}: WF_vars(Step(p)))
Live == <> (done = {1,2})

VARIABLE __position
__TraceInit == Init /\ __position = 0 /\ (turn = 0 /\ done = {})
__TraceNext == ((__position = 0 /\ __position' = 1 /\ (turn' = 1 /\ done' = {})) \/ (__position = 1 /\ __position' = 2 /\ (turn' = 0 /\ done' = {})) \/ (__position = 2 /\ __position' = 3 /\ (turn' = 0 /\ done' = {})) \/ (__position = 3 /\ __position' = 4 /\ (turn' = 1 /\ done' = {})) \/ (__position = 4 /\ __position' = 5 /\ (turn' = 0 /\ done' = {})) \/ (__position = 5 /\ __position' = 6 /\ (turn' = 0 /\ done' = {})) \/ (__position = 6 /\ __position' = 1 /\ (turn' = 1 /\ done' = {}))) /\ (Next \/ UNCHANGED <<turn,done>>)
__TraceSpec == __TraceInit /\ [][__TraceNext]_<<turn,done,__position>> /\ WF_<<turn,done,__position>>(__TraceNext)
__Expected == (Spec) /\ ~(Live)
====