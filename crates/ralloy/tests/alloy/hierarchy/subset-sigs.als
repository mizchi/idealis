sig Node { next: lone Node }
sig Marked in Node {}
sig Red, Blue in Node {}
one sig Head in Node {}
Overlap: run { some Red & Blue } for 2 but 0 Int
MarkedAll: run { Marked = Node and some Node } for 2 but 0 Int
assert MarkedInNode { Marked in Node }
MarkedHolds: check MarkedInNode for 3 but 0 Int
HeadAlone: run { no Node - Head } for 2 but 0 Int
NoHead: run { no Head } for 2 but 0 Int
HeadMarked: run { Head in Marked and Head in Red & Blue } for 2 but 0 Int
