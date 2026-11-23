sig A { f: set B }
sig B {}
DomainRestrict: run { some a: A | a <: f = a -> a.f and some a.f } for 2 but 0 Int
RangeRestrict: run { some b: B | f :> b != f } for 2 but 0 Int
Transpose: run { ~f = B <: ~f and some f } for 2 but 0 Int
Product: run { f = A -> B and some A and some B } for 2 but 0 Int
Univ: run { univ = A + B } for 2 but 0 Int
