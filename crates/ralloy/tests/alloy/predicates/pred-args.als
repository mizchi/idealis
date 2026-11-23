sig Node { edge: set Node }
pred connected[a, b: Node] { b in a.^edge }
pred selfLoop[n: Node] { n in n.edge }
SomeConnected: run { some a, b: Node | connected[a, b] } for 2 but 0 Int
RunWithParams: run connected for 2 but 0 Int
RunLoop: run selfLoop for 1 but 0 Int
assert Trans { all a, b, c: Node | connected[a, b] and connected[b, c] => connected[a, c] }
TransHolds: check Trans for 3 but 0 Int
Receiver: run { some a, b: Node | a.connected[b] and not b.connected[a] } for 2 but 0 Int
