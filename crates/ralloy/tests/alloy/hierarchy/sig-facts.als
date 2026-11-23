sig Node { succ: lone Node } { succ != this }
sig Box { items: set Item } { some items }
sig Item {}
SelfLoop: run { some n: Node | n.succ = n } for 2 but 0 Int
Chained: run { some succ } for 2 but 0 Int
EmptyBox: run { some b: Box | no b.items } for 2 but 0 Int
BoxesNeedItems: run { some Box and no Item } for 2 but 0 Int
assert NoLoops { no iden & succ }
NoLoopsHolds: check NoLoops for 3 but 0 Int
