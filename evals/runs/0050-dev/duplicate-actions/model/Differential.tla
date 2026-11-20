---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
First == x < 2 /\ x' = x + 1
Second == x < 2 /\ x' = x + 1
Next == First \/ Second
====