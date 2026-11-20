---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
I(p) == [<<4>> EXCEPT ![1] = p][1]
Init == x = 0
Next == UNCHANGED x
Safe == [<<9>> EXCEPT ![1] = I(CHOOSE n \in {9}: n = @)][1] = 4
====
