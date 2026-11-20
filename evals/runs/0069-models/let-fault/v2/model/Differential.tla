---- MODULE Differential ----

VARIABLE x

Init == x = FALSE
Next == x' = (LET left %% right == ~left IN x %% TRUE)
Safe == x = FALSE
====
