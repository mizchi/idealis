---- MODULE Differential ----
VARIABLE x
Init == x = 0
Next == CASE x = 5 -> x' = 1 [] x = 6 -> x' = 2
====