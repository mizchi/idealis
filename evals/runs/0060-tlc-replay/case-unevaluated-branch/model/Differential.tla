---- MODULE Differential ----
VARIABLE x
Init == x = IF TRUE THEN 0 ELSE CASE FALSE -> 1
Next == UNCHANGED x
====