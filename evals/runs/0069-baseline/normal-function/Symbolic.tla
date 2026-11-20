---- MODULE Symbolic ----
F(value) == value
f[key \in {1}] == F(key)
Main == f[1]
====
