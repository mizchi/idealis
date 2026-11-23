sig A {}
IntSize: run { #Int = 16 } for 1
SomeBetween: run { some i: Int | i > 2 and i < 4 } for 1 but 4 Int
NoneBetween: run { some i: Int | i > 2 and i < 3 } for 1 but 4 Int
TooNegative: run { some i: Int | i < -8 } for 1 but 4 Int
CountMatches: run { some i: Int | i = #A and i > 1 } for 3
