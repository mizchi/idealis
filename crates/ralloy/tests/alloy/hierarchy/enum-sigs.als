abstract sig Color {}
one sig Red, Green, Blue extends Color {}
sig Light { color: one Color }
sig Bulb, Lamp {}
Enum: run { Color = Red + Green + Blue } for 3 but 0 Int
NoOther: run { some c: Color | c not in Red + Green + Blue } for 3 but 0 Int
Merged: run { Red = Green } for 3 but 0 Int
TwoLights: run { some disj a, b: Light | a.color != b.color } for 3 but 0 Int
BulbsAndLamps: run { some Bulb and some Lamp and no Bulb & Lamp } for 1 but 0 Int
