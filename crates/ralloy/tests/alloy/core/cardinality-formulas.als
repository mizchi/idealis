sig A { r: set A }
OneOfTwo: run { one A } for 2 but exactly 2 A, 0 Int
OneOfOne: run { one A } for 2 but exactly 1 A, 0 Int
LoneOfOne: run { lone A } for 2 but exactly 1 A, 0 Int
LoneOfTwo: run { lone A } for 2 but exactly 2 A, 0 Int
OneEdge: run { one r and some A } for 2 but 0 Int
LoneLoop: run { lone r and r = A -> A } for 2 but exactly 2 A, 0 Int
