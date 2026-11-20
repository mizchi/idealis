---- MODULE Differential ----
VARIABLE x
Init == x \in {}
Next == UNCHANGED x
Inv == CASE FALSE -> TRUE
====