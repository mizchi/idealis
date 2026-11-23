sig S { a: set S, b: set S }
assert UnionCommutes { a + b = b + a }
assert DiffSubset { a - b in a }
assert InterLaw { a & b = a - (a - b) }
assert Wrong { a - b = b - a }
Union: check UnionCommutes for 3 but 0 Int
Diff: check DiffSubset for 3 but 0 Int
Inter: check InterLaw for 3 but 0 Int
Broken: check Wrong for 2 but 0 Int
