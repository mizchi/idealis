sig P {}
sig Q { r: P -> P }
assert Composition { all q: Q | q.r.(q.r) in P -> P }
CompositionHolds: check Composition for 3 but 0 Int
ThreeWay: run { some q: Q, p: P | q -> p -> p in r } for 2 but 0 Int
Transpose: run { some q: Q | ~(q.r) = q.r and some q.r } for 2 but 0 Int
Closure: run { some q: Q | some p: P | p in p.^(q.r) } for 2 but 0 Int
Inclusion: run { r in Q -> P -> P and some r } for 2 but 0 Int
Equality: run { some disj a, b: Q | a.r = b.r and some a.r } for 2 but 0 Int
