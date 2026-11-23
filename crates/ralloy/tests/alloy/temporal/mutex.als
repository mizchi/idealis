sig Proc {}
one sig Lock { var holder: lone Proc }
pred acquire[p: Proc] { no Lock.holder and Lock.holder' = p }
pred release[p: Proc] { Lock.holder = p and no Lock.holder' }
pred skip { Lock.holder' = Lock.holder }
fact { no Lock.holder }
fact { always { skip or (some p: Proc | acquire[p] or release[p]) } }
Safe: check { always { lone Lock.holder } } for 3 but 6 steps
Handover: run { some p, q: Proc | p != q and eventually { Lock.holder = p and eventually { Lock.holder = q } } } for 3 but 6 steps
NoHandoverWithOne: run { some p, q: Proc | p != q and eventually { Lock.holder = p and eventually { Lock.holder = q } } } for 3 but exactly 1 Proc, 6 steps
Released: check { all p: Proc | always { Lock.holder = p implies eventually { no Lock.holder } } } for 3 but 4 steps
OnlyFromFree: check { always { (some Lock.holder' and Lock.holder' != Lock.holder) implies no Lock.holder } } for 3 but 6 steps
