one sig Counter { var n: one Int }
fact { Counter.n = 0 }
fact { always { Counter.n' = plus[Counter.n, 1] } }
Wraps: run { eventually { Counter.n = -1 } } for 3 but 3 Int, 8 steps
NoLasso: run {} for 3 but 3 Int, 7 steps
AfterOne: check { after { Counter.n = 1 } } for 3 but 3 Int, 8 steps
Settles: check { eventually { always { Counter.n = 0 } } } for 3 but 3 Int, 8 steps
