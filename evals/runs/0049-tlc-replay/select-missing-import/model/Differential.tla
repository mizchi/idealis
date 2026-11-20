---- MODULE Differential ----
EXTENDS Integers
VARIABLE x
Keep(v) == TRUE
Init == x = (SelectSeq(<<1>>, Keep))
Next == UNCHANGED x
Safe == x = (TRUE)
====