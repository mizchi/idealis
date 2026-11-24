sig Person {}
sig Student extends Person {}
PlainPerson: run { some p: Person | p not in Student } for 2 but 0 Int
AllStudents: run { Person = Student and some Person } for 2 but 0 Int
ExactParent: run { no Student } for 2 but exactly 2 Person, 0 Int
ExactChild: run { Person = Student } for 3 but exactly 2 Student, 0 Int
ChildFillsParent: run { some disj a, b: Student | a in Student } for 1 but 0 Int
assert StudentsArePeople { Student in Person }
StudentsHold: check StudentsArePeople for 3 but 0 Int
