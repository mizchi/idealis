------------------------------ MODULE LongIfChain ------------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

VARIABLE
  (*
    @type: Int;
  *)
  x

(*
  @type: (() => Bool);
*)
step == x' := (IF x < 55 THEN x + 1 ELSE 0)

(*
  @type: (() => Int);
*)
label ==
  IF x = 0
  THEN 100
  ELSE IF x = 1
  THEN 101
  ELSE IF x = 2
  THEN 102
  ELSE IF x = 3
  THEN 103
  ELSE IF x = 4
  THEN 104
  ELSE IF x = 5
  THEN 105
  ELSE IF x = 6
  THEN 106
  ELSE IF x = 7
  THEN 107
  ELSE IF x = 8
  THEN 108
  ELSE IF x = 9
  THEN 109
  ELSE IF x = 10
  THEN 110
  ELSE IF x = 11
  THEN 111
  ELSE IF x = 12
  THEN 112
  ELSE IF x = 13
  THEN 113
  ELSE IF x = 14
  THEN 114
  ELSE IF x = 15
  THEN 115
  ELSE IF x = 16
  THEN 116
  ELSE IF x = 17
  THEN 117
  ELSE IF x = 18
  THEN 118
  ELSE IF x = 19
  THEN 119
  ELSE IF x = 20
  THEN 120
  ELSE IF x = 21
  THEN 121
  ELSE IF x = 22
  THEN 122
  ELSE IF x = 23
  THEN 123
  ELSE IF x = 24
  THEN 124
  ELSE IF x = 25
  THEN 125
  ELSE IF x = 26
  THEN 126
  ELSE IF x = 27
  THEN 127
  ELSE IF x = 28
  THEN 128
  ELSE IF x = 29
  THEN 129
  ELSE IF x = 30
  THEN 130
  ELSE IF x = 31
  THEN 131
  ELSE IF x = 32
  THEN 132
  ELSE IF x = 33
  THEN 133
  ELSE IF x = 34
  THEN 134
  ELSE IF x = 35
  THEN 135
  ELSE IF x = 36
  THEN 136
  ELSE IF x = 37
  THEN 137
  ELSE IF x = 38
  THEN 138
  ELSE IF x = 39
  THEN 139
  ELSE IF x = 40
  THEN 140
  ELSE IF x = 41
  THEN 141
  ELSE IF x = 42
  THEN 142
  ELSE IF x = 43
  THEN 143
  ELSE IF x = 44
  THEN 144
  ELSE IF x = 45
  THEN 145
  ELSE IF x = 46
  THEN 146
  ELSE IF x = 47
  THEN 147
  ELSE IF x = 48
  THEN 148
  ELSE IF x = 49
  THEN 149
  ELSE IF x = 50
  THEN 150
  ELSE IF x = 51 THEN 151 ELSE IF x = 52 THEN 152 ELSE IF x = 53 THEN 153 ELSE 0

(*
  @type: (() => Bool);
*)
init == x = 0

(*
  @type: (() => Bool);
*)
safety == label >= 0

(*
  @type: (() => Bool);
*)
broken == label /= 153

(*
  @type: (() => Bool);
*)
q_init == init

(*
  @type: (() => Bool);
*)
q_step == step

(*
  @type: (() => Bool);
*)
q_inv == safety /\ broken

================================================================================
