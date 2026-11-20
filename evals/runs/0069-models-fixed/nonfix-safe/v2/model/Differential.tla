---- MODULE Differential ----

VARIABLE x
left %% right == ~left
Init == x = FALSE
Next == x' = (%%(x, TRUE))
Safe == x \in BOOLEAN
====
