---- MODULE Differential ----
EXTENDS Naturals, Sequences
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Wrap(p) == \A c \in {9}: p
Safe == \A c \in {0,1}: LET G(q) == Wrap(q /\ (c \in {0,1})) IN G(\A z \in {0}: z = 0)
====
