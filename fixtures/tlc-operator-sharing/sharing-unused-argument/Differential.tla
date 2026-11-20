---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Ignore(p) == TRUE
Safe == Ignore(Head(<<>>))
====
