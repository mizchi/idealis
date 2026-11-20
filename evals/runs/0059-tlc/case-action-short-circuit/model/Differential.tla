---- MODULE Differential ----
EXTENDS Sequences
VARIABLE x
Init == x = 0
Next == CASE TRUE -> UNCHANGED x [] (Head(<<>>) = 0) -> x' = 9
====