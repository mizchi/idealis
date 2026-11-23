sig Account { balance: one Int }
Positive: run { some Account and all a: Account | a.balance > 0 } for 2 but 3 Int
SumFive: run { some disj a, b: Account | plus[a.balance, b.balance] = 5 } for 2 but 4 Int
Zero: run { some a: Account | a.balance = 0 } for 1 but 3 Int
TooLarge: run { some a: Account | a.balance > 7 } for 1 but 4 Int
assert Bounded { all a: Account | a.balance <= 7 }
BoundedHolds: check Bounded for 2 but 4 Int
Distinct: run { #Account = 2 and #Account.balance = 1 } for 2 but 3 Int
