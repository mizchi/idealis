-------------------------------- MODULE SetFold --------------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

VARIABLE
  (*
    @type: Int;
  *)
  total

(*
  @type: (() => Bool);
*)
init ==
  total
    = (LET (*
      @type: ((Int, Int) => Int);
    *)
    __QUINT_LAMBDA0(acc_13, element_13) == acc_13 + element_13
    IN
    ApaFoldSet(__QUINT_LAMBDA0, 0, { 3, 1, 2 }))

(*
  @type: (() => Bool);
*)
step == total' := total

(*
  @type: (() => Bool);
*)
safety == total = 6

(*
  @type: (() => Bool);
*)
broken == total = 5

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
