sig X {}
one sig R { t: X -> X -> X }
sig Y { u: X -> X }
RightJoin: run { some x: X | some (R.t).x and (R.t).x != x.(R.t) } for 2 but 0 Int
FieldRightJoin: run { some x: X | some u.x and no x.u } for 2 but 0 Int
MiddleJoin: run { some disj a, b: X | a.((R.t).b) != b.((R.t).a) } for 2 but 0 Int
assert RightTyped { all x: X | (R.t).x in X -> X }
RightTypedHolds: check RightTyped for 2 but 0 Int
TernaryJoinBinary: run { some y: Y | some (R.t).(y.u) } for 2 but 0 Int
