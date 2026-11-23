sig Owner { one_: one Resource, maybe: lone Resource, many: some Resource }
sig Resource {}
NoOwners: run { no Owner } for 2 but 0 Int
OneOwner: run { one Owner } for 2 but 0 Int
SharedOne: run { some disj a, b: Owner | a.one_ = b.one_ } for 2 but 0 Int
EmptyMaybe: run { some o: Owner | no o.maybe } for 1 but 0 Int
EmptyMany: run { some o: Owner | no o.many } for 2 but 0 Int
