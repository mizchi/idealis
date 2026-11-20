------------------------------ MODULE NestedFold ------------------------------

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
    __QUINT_LAMBDA1(acc_24, outer_24) ==
      acc_24
        + (LET (*
          @type: ((Int, Int) => Int);
        *)
        __QUINT_LAMBDA0(inner_21, element_21) == inner_21 + element_21
        IN
        ApaFoldSet(__QUINT_LAMBDA0, 0, { outer_24, (outer_24 + 1) }))
    IN
    ApaFoldSeqLeft(__QUINT_LAMBDA1, 0, <<1, 2>>))

(*
  @type: (() => Bool);
*)
step == total' := total

(*
  @type: (() => Bool);
*)
safety == total = 8

(*
  @type: (() => Bool);
*)
broken == total = 6

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
