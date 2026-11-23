abstract sig Animal { eats: set Animal }
sig Cat, Dog extends Animal {}
Partition: run { some Cat and some Dog } for 2 but 0 Int
NoPlain: run { some a: Animal | a not in Cat + Dog } for 3 but 0 Int
ThreeCats: run { some disj a, b, c: Cat | a in Cat } for 2 but 0 Int
ChildScope: run { some disj a, b, c: Cat | a in Cat } for 2 but 3 Cat, 0 Int
EatsDogs: run { all c: Cat | some c.eats & Dog } for 2 but 0 Int
assert Disjoint { no Cat & Dog }
DisjointHolds: check Disjoint for 3 but 0 Int
