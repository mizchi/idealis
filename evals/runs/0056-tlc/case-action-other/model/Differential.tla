---- MODULE Differential ----
EXTENDS Sequences
VARIABLE x
Init == x = 0
Next == CASE FALSE -> x' = 9 [] OTHER -> UNCHANGED x
====