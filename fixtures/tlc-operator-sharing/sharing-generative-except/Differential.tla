---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
F(v, a) == IF ENABLED (UNCHANGED v) THEN [<<4>> EXCEPT ![1] = a] ELSE <<0>>
Init == x = 0
Next == UNCHANGED x
Safe == x = 0 /\ [<<9>> EXCEPT ![1] = F(x, @)[1]][1] = 4
====
