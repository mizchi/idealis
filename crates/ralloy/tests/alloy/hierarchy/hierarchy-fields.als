sig Account { owner: one Person }
abstract sig Person {}
sig Customer extends Person { accounts: set Account }
sig Staff extends Person {}
Consistent: run { some accounts and all c: Customer | all a: c.accounts | a.owner = c } for 2 but 0 Int
assert OwnerTyped { Account.owner in Customer + Staff }
OwnerHolds: check OwnerTyped for 3 but 0 Int
StaffOwns: run { some a: Account | a.owner in Staff } for 2 but 0 Int
assert AccountsOwned { all c: Customer | c.accounts.owner in c }
AccountsFail: check AccountsOwned for 2 but 0 Int
