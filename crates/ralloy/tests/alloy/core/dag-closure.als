sig Node { edge: set Node }
fact Dag { no iden & ^edge }
assert NoReturn { all n: Node | n not in n.^edge }
assert Reflexive { iden & (Node -> Node) in *edge }
Safe: check NoReturn for 3 but 0 Int
ReflexiveClosure: check Reflexive for 3 but 0 Int
Chain: run { some a, b, c: Node | a -> b in edge and b -> c in edge } for 3 but 0 Int
