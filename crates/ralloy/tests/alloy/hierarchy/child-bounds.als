sig Person {}
sig Student extends Person {}
abstract sig Shape {}
sig Circle, Square extends Shape {}
TwoStudents: run { some disj a, b: Student | a in Student } for 3 but 1 Student, 0 Int
OneStudent: run { some Student and some Person - Student } for 3 but 1 Student, 0 Int
TwoCircles: run { some disj a, b: Circle | a in Circle } for 3 but 1 Circle, 0 Int
SquaresFill: run { #Square = 2 and some Circle } for 3 but 1 Circle
