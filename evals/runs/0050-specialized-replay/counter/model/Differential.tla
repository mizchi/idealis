---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Advance == x < 3 /\ x' = x + 1
Stay == UNCHANGED x
Unused == FALSE /\ x' = 0
Next == Advance \/ Stay \/ Unused
====