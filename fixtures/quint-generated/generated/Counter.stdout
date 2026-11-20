-------------------------------- MODULE Counter --------------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

VARIABLE
  (*
    @type: Int;
  *)
  count

(*
  @type: (() => Bool);
*)
step == count' := (IF count < 3 THEN count + 1 ELSE 0)

(*
  @type: (() => Bool);
*)
safety == count >= 0 /\ count <= 3

(*
  @type: (() => Bool);
*)
broken == count < 2

(*
  @type: (() => Bool);
*)
init == count = 0

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
