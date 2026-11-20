---- MODULE Differential ----
EXTENDS Integers, Sequences
VARIABLE x
F(p) == x \notin p
Init == x = 0
Next == UNCHANGED x
Safe == ~F(Nat)
====
