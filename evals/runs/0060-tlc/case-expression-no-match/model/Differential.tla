---- MODULE Differential ----
VARIABLE x
Init == x = CASE FALSE -> 1 [] FALSE -> 2
Next == UNCHANGED x
====