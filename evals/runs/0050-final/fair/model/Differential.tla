---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Advance == x < 3 /\ x' = x + 1
Stay == UNCHANGED x
Unused == FALSE /\ x' = 0
Next == Advance \/ Stay \/ Unused
Spec == Init /\ [][Next]_x /\ WF_x(Advance)
Live == <> (x = 3)
====