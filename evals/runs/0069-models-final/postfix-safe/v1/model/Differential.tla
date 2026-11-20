---- MODULE Differential ----

VARIABLE x
value ^+ == ~value
Init == x = FALSE
Next == x' = (x^+)
Safe == x \in BOOLEAN
====
