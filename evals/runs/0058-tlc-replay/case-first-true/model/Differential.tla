---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = CASE FALSE -> 5 [] TRUE -> 0 [] TRUE -> 9
Next == CASE x = 0 -> x' = 1 [] x = 1 -> x' = 2 [] TRUE -> UNCHANGED x [] TRUE -> x' = 9
Safe == x < 3
====