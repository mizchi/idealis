sig Pigeon { hole: one Hole }
sig Hole {}
fact Injective { all disj p, q: Pigeon | p.hole != q.hole }
Fit: run { } for 3 but exactly 3 Pigeon, exactly 3 Hole, 0 Int
Overflow: run { } for 3 but exactly 3 Pigeon, exactly 2 Hole, 0 Int
assert Distinct { all disj p, q: Pigeon | p.hole != q.hole }
Holds: check Distinct for 3 but 0 Int
