---- MODULE Differential ----
EXTENDS Integers
VARIABLES turn, done
vars == <<turn,done>>
Init == turn = 0 /\ done = {}
Tick == turn' = 1-turn /\ UNCHANGED done
Step(p) == turn = 1 /\ p \notin done /\ turn' = 0 /\ done' = done \cup {p}
Next == Tick \/ (\E p \in {1,2}: Step(p))
Spec == Init /\ [][Next]_vars /\ FairTick /\ (\A p \in {1,2}: WF_vars(Step(p)))
FairTick == WF_vars(Tick)
Live == <> (done = {1,2})
====