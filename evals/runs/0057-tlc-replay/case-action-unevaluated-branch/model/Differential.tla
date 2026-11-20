---- MODULE Differential ----
EXTENDS Sequences
VARIABLE x
Init == x = 0
Next == IF TRUE THEN UNCHANGED x ELSE CASE FALSE -> x' = 9
====