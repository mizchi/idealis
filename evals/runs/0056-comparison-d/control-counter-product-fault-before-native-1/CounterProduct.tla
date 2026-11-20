---- MODULE CounterProduct ----
EXTENDS Naturals
CONSTANT M
VARIABLES a,b,c,d
Init == a=0 /\ b=0 /\ c=0 /\ d=0
Stepa == a' = (IF a<M-1 THEN a+1 ELSE 0) /\ UNCHANGED <<b,c,d>>
Stepb == b' = (IF b<M-1 THEN b+1 ELSE 0) /\ UNCHANGED <<a,c,d>>
Stepc == c' = (IF c<M-1 THEN c+1 ELSE 0) /\ UNCHANGED <<a,b,d>>
Stepd == d' = (IF d<M-1 THEN d+1 ELSE 0) /\ UNCHANGED <<a,b,c>>
Next == Stepa \/ Stepb \/ Stepc \/ Stepd
Safe == (a \in 0..(M-1) /\ b \in 0..(M-1) /\ c \in 0..(M-1) /\ d \in 0..(M-1)) /\ (a+b+c+d < 4*(M-1))
====