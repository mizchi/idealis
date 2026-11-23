sig S { f: set S }
pred related[x: S, y: S] { x -> y in f }
pred sym[x, y: S] { related[x, y] <=> related[y, x] }
fun pairs: S -> S { f & ~f }
assert SymSelf { all x: S | sym[x, x] }
SymSelfHolds: check SymSelf for 3 but 0 Int
AsymPair: run { some x, y: S | not sym[x, y] } for 2 but 0 Int
SymPairs: run { some pairs and pairs = f } for 2 but 0 Int
assert PairsSymmetric { pairs = ~pairs }
PairsHold: check PairsSymmetric for 3 but 0 Int
