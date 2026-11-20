---- MODULE Diff ----
EXTENDS Naturals
VARIABLES x, stable
Init == /\ x = 0
        /\ stable = "constant"
Next == /\ x' = x + 1
        /\ UNCHANGED stable
Safe == x < 2
First == x = 0
Immediate == x = 1
NextStable == UNCHANGED <<x, stable>>
Future == <>(x = 5)
====
