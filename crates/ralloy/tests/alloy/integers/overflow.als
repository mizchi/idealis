sig A {}
Overflow: run { #A = 4 } for 4 but 3 Int
Wrap: run { plus[3, 1] = -4 } for 1 but 3 Int
Max: run { plus[2, 1] = 3 } for 1 but 3 Int
CountWrap: run { some A and #A < 0 } for 5 but 3 Int
NegWrap: run { minus[-4, 1] = 3 } for 1 but 3 Int
