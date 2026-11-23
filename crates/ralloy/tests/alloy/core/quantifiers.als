sig A { r: set A }
AllSome: run { all x: A | some x.r } for 2 but exactly 2 A, 0 Int
NoneLoop: run { no x: A | x in x.r } for 2 but 0 Int
OneMap: run { one x: A | x.r = A } for 2 but exactly 2 A, 0 Int
LoneMap: run { lone x: A | some x.r } for 2 but exactly 2 A, 0 Int
DisjointPair: run { some disj x, y: A | x.r = y and y.r = x } for 2 but 0 Int
EmptyAll: run { all x: A | x in none } for 2 but 0 Int
