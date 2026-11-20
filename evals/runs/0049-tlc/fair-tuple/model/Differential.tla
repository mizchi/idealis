---- MODULE Differential ----
EXTENDS Integers
VARIABLE done
Init == done = {}
Step(p) == p \notin done /\ done' = done \cup {p}
Next == \E p \in {1,2}: Step(p)
Fair(processes) == \A p \in processes: WF_done(Step(p))
Spec == Init /\ [][Next]_done /\ (\A <<p,q>> \in {<<1,0>>,<<2,0>>}: WF_done(Step(p+q)))
Live == <> (done = {1,2})
====