sig A { r: set A }
fun out[x: A]: set A { x.r }
pred reaches[x, y: A] { y in out[x] + out[out[x]] }
pred symmetric { all x, y: A | reaches[x, y] => reaches[y, x] }
fun both[x: A, y: A]: set A { out[x] & out[y] }
Sym: run { symmetric and some r } for 2 but 0 Int
TwoStep: run { some x, y: A | reaches[x, y] and y not in out[x] } for 3 but 0 Int
Common: run { some disj x, y: A | some both[x, y] } for 2 but 0 Int
assert ReachOut { all x, y: A | y in out[x] => reaches[x, y] }
ReachOutHolds: check ReachOut for 3 but 0 Int
