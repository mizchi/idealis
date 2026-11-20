---- MODULE WideFrontier ----
EXTENDS Integers

VARIABLE x

Init == x = 0

Next == \E a \in 1..4 : \E b \in 0..63 : b >= 0 /\ b <= 63 /\ b # 64 /\ b # 65 /\ b # 66 /\ b # 67 /\ b # 68 /\ b # 69 /\ x' = b

Safe == x \in 0..63

====
