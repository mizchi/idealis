----------------------------- MODULE SequenceFold -----------------------------

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
    __QUINT_LAMBDA0(acc_15, element_15) == acc_15 * 10 + element_15
    IN
    ApaFoldSeqLeft(__QUINT_LAMBDA0, 0, <<3, 1, 2>>))

(*
  @type: (() => Bool);
*)
step == total' := total

(*
  @type: (() => Bool);
*)
safety == total = 312

(*
  @type: (() => Bool);
*)
broken == total = 123

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
