sig X {}
one sig R { t: X -> X -> X }
ProductArity: run { R.t = X -> X -> X and some X } for 2 but 0 Int
DomainRestrict: run { some x: X | x <: (R.t) != R.t } for 2 but 0 Int
RangeRestrict: run { some x: X | (R.t) :> x = R.t and some R.t } for 2 but exactly 2 X, 0 Int
UnionDiff: run { some x: X | (R.t) - (x -> X -> X) + (x -> x -> x) = R.t and some R.t } for 2 but 0 Int
Intersection: run { some R.t & (X -> X -> X) } for 2 but 0 Int
assert JoinTwice { all a, b: X | b.(a.(R.t)) in X }
JoinTwiceHolds: check JoinTwice for 2 but 0 Int
