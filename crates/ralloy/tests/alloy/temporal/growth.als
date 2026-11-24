sig Item {}
one sig Box { var items: set Item }
pred grow { some i: Item - Box.items | Box.items' = Box.items + i }
pred stay { Box.items' = Box.items }
Grow: run { no Box.items and always { grow } } for 3 but 5 steps
GrowOrStay: run { no Box.items and always { grow or stay } and eventually { #Box.items = 3 } } for 3 but 5 steps
TooFew: run { no Box.items and always { grow or stay } and eventually { #Box.items = 3 } } for 3 but exactly 2 Item, 5 steps
ShortTrace: run { no Box.items and always { grow or stay } and eventually { #Box.items = 3 } } for 3 but 2 steps
Monotone: check { (no Box.items and always { grow or stay }) implies always { Box.items in Box.items' } } for 3 but 5 steps
