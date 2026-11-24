sig A {}
Plus: run { #A = plus[1, 1] } for 3
Minus: run { #A = minus[3, 1] and some A } for 3
Mul: run { #A = mul[2, 2] } for 4
Neg: run { negate[#A] = -2 } for 3
Div: run { div[#A, 2] = 1 and #A = 3 } for 3
Rem: run { rem[#A, 2] = 1 and #A > 1 } for 3
Impossible: run { plus[#A, 1] = 0 } for 3
