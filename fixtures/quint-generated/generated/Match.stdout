--------------------------------- MODULE Match ---------------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

(*
  @type: (() => Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str }));
*)
Busy == Variant("Busy", [tag |-> "UNIT"])

(*
  @type: (() => Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str }));
*)
Done == Variant("Done", [tag |-> "UNIT"])

VARIABLE
  (*
    @type: Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str });
  *)
  phase

(*
  @type: (() => Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str }));
*)
Idle == Variant("Idle", [tag |-> "UNIT"])

(*
  @type: (() => Bool);
*)
init == phase = Idle

(*
  @type: (() => Bool);
*)
step ==
  phase'
    := (CASE VariantTag(phase) = "Idle"
        -> LET (*
          @type: (({ tag: Str }) => Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str }));
        *)
        __QUINT_LAMBDA0(id__30) == Busy
        IN
        __QUINT_LAMBDA0((VariantGetUnsafe("Idle", phase)))
      [] VariantTag(phase) = "Busy"
        -> LET (*
          @type: (({ tag: Str }) => Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str }));
        *)
        __QUINT_LAMBDA1(id__33) == Done
        IN
        __QUINT_LAMBDA1((VariantGetUnsafe("Busy", phase)))
      [] VariantTag(phase) = "Done"
        -> LET (*
          @type: (({ tag: Str }) => Busy({ tag: Str }) | Done({ tag: Str }) | Idle({ tag: Str }));
        *)
        __QUINT_LAMBDA2(id__36) == Idle
        IN
        __QUINT_LAMBDA2((VariantGetUnsafe("Done", phase))))

(*
  @type: (() => Bool);
*)
safety ==
  CASE VariantTag(phase) = "Idle"
      -> LET (*
        @type: (({ tag: Str }) => Bool);
      *)
      __QUINT_LAMBDA3(id__47) == TRUE
      IN
      __QUINT_LAMBDA3((VariantGetUnsafe("Idle", phase)))
    [] VariantTag(phase) = "Busy"
      -> LET (*
        @type: (({ tag: Str }) => Bool);
      *)
      __QUINT_LAMBDA4(id__50) == TRUE
      IN
      __QUINT_LAMBDA4((VariantGetUnsafe("Busy", phase)))
    [] VariantTag(phase) = "Done"
      -> LET (*
        @type: (({ tag: Str }) => Bool);
      *)
      __QUINT_LAMBDA5(id__53) == TRUE
      IN
      __QUINT_LAMBDA5((VariantGetUnsafe("Done", phase)))

(*
  @type: (() => Bool);
*)
broken == phase /= Done

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
