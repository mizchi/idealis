sig A {}
sig B { f: set A }
TwoA: run { #A = 2 } for 3
NoFour: run { #A = 4 } for 3
CompareCounts: run { #A > #B and some B } for 3
FieldCount: run { some b: B | #b.f = 3 } for 3
LessEq: run { #A <= 1 and #B >= 2 } for 3
OtherLessEq: run { #A =< 0 and #B > 2 } for 3
assert CountBound { #A <= 3 }
CountHolds: check CountBound for 3
assert CountPositive { #A > 0 }
CountFails: check CountPositive for 3
