sig Node { next: lone Node }
pred Ring { all n: Node | Node in n.^next }
pred Line { some n: Node | no n.next }
fact { some Node }
RingExists: run Ring for 3 but 0 Int
LineExists: run Line for 3 but 0 Int
assert RingNoEnd { Ring => no n: Node | no n.next }
RingHasNoEnd: check RingNoEnd for 3 but 0 Int
