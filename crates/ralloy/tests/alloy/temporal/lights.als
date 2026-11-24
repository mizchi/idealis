abstract sig Color {}
one sig Red, Green extends Color {}
one sig Light { var color: one Color }
fact { Light.color = Red }
fact { always { Light.color' != Light.color } }
Alternates: check { always { Light.color = Red implies after { Light.color = Green } } } for 3 but 4 steps
StaysRed: check { always { Light.color = Red } } for 3 but 4 steps
GreenEventually: check { eventually { Light.color = Green } } for 3 but 4 steps
SomeTrace: run {} for 3 but 4 steps
InitialOnly: run { Light.color = Green } for 3 but 4 steps
