---- MODULE Differential ----
EXTENDS Sequences
VARIABLE x
left + right == ~left
Init == x = FALSE
Next == x' = (x + TRUE)
Safe == x \in BOOLEAN
====
