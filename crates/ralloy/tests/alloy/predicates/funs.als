sig Person { parent: set Person }
fun ancestors[p: Person]: set Person { p.^parent }
fun roots: set Person { Person - parent.Person }
fun grand: Person -> Person { parent.parent }
assert Acyclic { all p: Person | p not in ancestors[p] }
AcyclicFails: check Acyclic for 2 but 0 Int
GrandBinary: run { some grand } for 3 but 0 Int
Roots: run { some roots and some Person - roots } for 2 but 0 Int
ReceiverFun: run { some p: Person | some p.ancestors } for 2 but 0 Int
BoxJoin: run { some p: Person | some parent[p] and parent[p] != p.parent } for 2 but 0 Int
