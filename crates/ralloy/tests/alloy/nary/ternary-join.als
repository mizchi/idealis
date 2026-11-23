sig Account {}
sig Day {}
one sig Bank { balance: Account -> Day }
Empty: run { no Bank.balance } for 2 but 0 Int
Full: run { Bank.balance = Account -> Day and some Account and some Day } for 2 but 0 Int
JoinLeft: run { some a: Account | some a.(Bank.balance) } for 2 but 0 Int
JoinRight: run { some d: Day | some (Bank.balance).d and no Account - (Bank.balance).d } for 2 but exactly 2 Account, 0 Int
assert Typed { Bank.balance in Account -> Day }
TypedHolds: check Typed for 3 but 0 Int
assert AllPairs { Bank.balance = Account -> Day }
AllPairsFails: check AllPairs for 2 but 0 Int
