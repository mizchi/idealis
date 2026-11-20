---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
P(p) == CHOOSE n \in {4}: p
Init == x = 0
Next == UNCHANGED x
Safe == \A n \in {9}: P(n = 9) = 0
====
