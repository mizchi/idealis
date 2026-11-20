---- MODULE Values ----
VARIABLE v
Init == v = <<TRUE, [tag |-> "ok"], {1, 2}>>
Next == UNCHANGED v
Safe == v = <<>>
Rejected == FALSE
====
