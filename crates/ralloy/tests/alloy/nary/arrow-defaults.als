sig A {}
sig B {}
sig S { f: A some -> set B, g: A set -> one B }
SomeLeft: run { some s: S | some s.f } for 2 but 0 Int
assert SomeEveryB { all s: S, b: B | some (s.f).b }
SomeEveryBHolds: check SomeEveryB for 2 but 0 Int
assert OneEveryA { all s: S, a: A | one a.(s.g) }
OneEveryAHolds: check OneEveryA for 2 but 0 Int
NoBButA: run { some A and no B and some S } for 2 but 0 Int
