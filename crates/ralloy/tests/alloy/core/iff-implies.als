sig P { q: set P }
assert Contrapositive { (some q => some P) <=> (no P => no q) }
assert BadConverse { (some q => some P) <=> (some P => some q) }
Contra: check Contrapositive for 3 but 0 Int
Converse: check BadConverse for 2 but 0 Int
Iff: run { some q <=> no P } for 2 but 0 Int
