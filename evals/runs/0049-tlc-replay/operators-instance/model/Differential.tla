---- MODULE Differential ----
VARIABLE x
A == INSTANCE Operators WITH state <- x, Limit <- 1
Init == A!Init
Next == A!Next
Safe == A!Safe
====