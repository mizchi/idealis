sig A {}
sig B {}
sig S { f: A one -> lone B }
Functional: run { some s: S | some s.f } for 2 but 0 Int
assert EachAOnce { all s: S, a: A | lone a.(s.f) }
EachAOnceHolds: check EachAOnce for 3 but 0 Int
assert Total { all s: S, b: B | one (s.f).b }
TotalHolds: check Total for 3 but 0 Int
