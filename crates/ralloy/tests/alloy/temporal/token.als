sig Node { edges: set Node }
one sig Token { var at: one Node }
fact { always { Token.at' in Token.at.edges } }
Cycle: run { some n: Node | always { eventually { Token.at = n } } } for 3 but 4 steps
VisitAll: run { #Node = 3 and (all n: Node | eventually { Token.at = n }) } for 3 but 3 steps
VisitAllShort: run { #Node = 3 and (all n: Node | eventually { Token.at = n }) } for 3 but 2 steps
Stuck: run { no edges } for 3 but 4 steps
Reachable: check { all n: Node | Token.at = n implies always { Token.at in n.*edges } } for 3 but 4 steps
Unreachable: check { all n: Node | Token.at = n implies always { Token.at in n.^edges } } for 3 but 4 steps
