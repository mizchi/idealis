sig V { e: set V }
fact Symmetric { e = ~e }
fact Irreflexive { no iden & e }
Triangle: run { some disj a, b, c: V | a -> b + b -> c + c -> a in e } for 4 but 0 Int
assert Connected { all disj a, b: V | a -> b in ^e }
NotConnected: check Connected for 4 but 0 Int
Matching: run { all v: V | one v.e } for 4 but exactly 4 V, 0 Int
