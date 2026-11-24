abstract sig Shape {}
abstract sig Polygon extends Shape {}
sig Triangle, Square extends Polygon {}
sig Circle extends Shape {}
Levels: run { some Triangle and some Square and some Circle } for 3 but 0 Int
PolygonOnly: run { some Polygon - Triangle - Square } for 3 but 0 Int
Bounded: run { some disj a, b, c, d: Shape | a in Shape } for 3 but 0 Int
assert Layers { Triangle + Square in Polygon and Polygon + Circle = Shape }
LayersHold: check Layers for 3 but 0 Int
ExactSquares: run { no Triangle and no Circle } for 3 but exactly 2 Square, 0 Int
