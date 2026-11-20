---- MODULE Differential ----
EXTENDS Naturals
VARIABLES visible,flag0
Init == visible = 0 /\ flag0 = FALSE
Next == (/\ ~flag0 /\ flag0' = TRUE /\ UNCHANGED <<visible>>) \/ (visible' = (IF visible < 3 THEN visible + 1 ELSE IF flag0 THEN 0 ELSE 0) /\ UNCHANGED <<flag0>>)
Safe == visible <= 3
====