sig A {}
sig B { to: set A }
ExactThree: run { all b: B | b.to = A } for 4 but exactly 3 A, exactly 2 B, 0 Int
Empty: run { no A and no B } for 2 but 0 Int
Zero: run { some A } for 0 but 0 Int
