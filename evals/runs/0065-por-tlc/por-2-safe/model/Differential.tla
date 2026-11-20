---- MODULE Differential ----
EXTENDS Naturals
VARIABLES visible,flag0,flag1
Init == visible = 0 /\ flag0 = FALSE /\ flag1 = FALSE
Next == (/\ ~flag0 /\ flag0' = TRUE /\ UNCHANGED <<visible,flag1>>) \/ (/\ ~flag1 /\ flag1' = TRUE /\ UNCHANGED <<visible,flag0>>) \/ (visible' = (IF visible < 3 THEN visible + 1 ELSE 0) /\ UNCHANGED <<flag0,flag1>>)
Safe == visible <= 3
====