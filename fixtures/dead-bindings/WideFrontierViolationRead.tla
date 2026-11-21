---- MODULE WideFrontierViolationRead ----
EXTENDS Integers

VARIABLE x

Init == x = 0

Next == \E a \in 1..4 : \E b \in 0..63 : a > 0 /\ b >= 0 /\ b <= 63 /\ b # 64 /\ b # 65 /\ b # 66 /\ b # 67 /\ b # 68 /\ b # 69 /\ x' = IF x = 0 THEN b ELSE b + 64

Safe == x \in 0..63

====
