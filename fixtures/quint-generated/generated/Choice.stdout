-------------------------------- MODULE Choice --------------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

VARIABLE
  (*
    @type: Int;
  *)
  value

(*
  @type: (() => Bool);
*)
step == \E selected \in { 0, 1, 2 }: value' := selected

(*
  @type: (() => Bool);
*)
safety == value >= 0 /\ value <= 2

(*
  @type: (() => Bool);
*)
broken == value < 2

(*
  @type: (() => Bool);
*)
init == value = 0

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
