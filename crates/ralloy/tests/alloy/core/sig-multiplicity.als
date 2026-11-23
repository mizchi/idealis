one sig Root { child: set Leaf }
lone sig Spare {}
some sig Leaf {}
RootOnly: run { no Leaf } for 3 but 0 Int
SpareAbsent: run { no Spare } for 3 but 0 Int
AllChildren: run { Root.child = Leaf } for 3 but 0 Int
