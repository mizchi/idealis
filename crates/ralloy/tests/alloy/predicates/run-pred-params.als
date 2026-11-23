sig Item { price: one Tier }
abstract sig Tier {}
one sig Low, High extends Tier {}
pred expensive[i: Item] { i.price = High }
pred mixed[a, b: Item] { a.price != b.price }
pred samePrice[disj a, b: Item] { a.price = b.price }
Expensive: run expensive for 2 but 0 Int
Mixed: run mixed for 2 but 0 Int
MixedOne: run mixed for 1 but 0 Int
DisjPair: run samePrice for 1 but 0 Int
DisjPairTwo: run samePrice for 2 but 0 Int
