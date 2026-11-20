---- MODULE Shadow3 ----
EXTENDS Naturals, Sequences
Len == [w \in {1, 2} |-> 3]
VARIABLES s
Init == s = <<>>
Next == Len(s) < Len[1] /\ s' = Append(s, 1)
====
