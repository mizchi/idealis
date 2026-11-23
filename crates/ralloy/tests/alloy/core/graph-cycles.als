sig Node { edge: set Node }
fact NoSelfLoops { no iden & edge }
assert Acyclic { no iden & ^edge }
Cyclic: check Acyclic for 3 but 0 Int
TwoCycle: run { some n: Node | n in n.^edge } for 2 but 0 Int
Dag: run { no iden & ^edge and some edge } for 3 but 0 Int
