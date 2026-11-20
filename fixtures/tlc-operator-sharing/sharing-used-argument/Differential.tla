---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Use(p) == p = 0
Safe == Use(Head(<<>>))
====
