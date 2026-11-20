---- MODULE Differential ----
EXTENDS Naturals
VARIABLE x
Init == x = 1 \div 0
Next == UNCHANGED x
====