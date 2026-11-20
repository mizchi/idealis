---- MODULE Differential ----
EXTENDS Naturals
VARIABLES visible,flag0,flag1,flag2
Init == visible = 0 /\ flag0 = FALSE /\ flag1 = FALSE /\ flag2 = FALSE
Next == (/\ ~flag0 /\ flag0' = TRUE /\ UNCHANGED <<visible,flag1,flag2>>) \/ (/\ ~flag1 /\ flag1' = TRUE /\ UNCHANGED <<visible,flag0,flag2>>) \/ (/\ ~flag2 /\ flag2' = TRUE /\ UNCHANGED <<visible,flag0,flag1>>) \/ (visible' = (IF visible < 3 THEN visible + 1 ELSE 0) /\ UNCHANGED <<flag0,flag1,flag2>>)
Safe == visible <= 3
====