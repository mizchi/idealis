---------------------------- MODULE NondetTagsRead ----------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

(*
  @type: (() => Set(Int));
*)
Ids == 1 .. 12

(*
  @type: (() => Set(Str));
*)
Tags ==
  { "request",
    "reply",
    "failure",
    "cancel",
    "reset",
    "idle1",
    "idle2",
    "idle3",
    "idle4" }

VARIABLE
  (*
    @type: Set(Int);
  *)
  pending

VARIABLE
  (*
    @type: Int;
  *)
  done

VARIABLE
  (*
    @type: Str;
  *)
  last

(*
  @type: ((Int) => Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str }));
*)
Request(__RequestParam_14) == Variant("Request", __RequestParam_14)

(*
  @type: (({ id: Int, ok: Bool }) => Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str }));
*)
Reply(__ReplyParam_20) == Variant("Reply", __ReplyParam_20)

(*
  @type: ((Set(Int), Int, Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str })) => <<Set(Int), Int>>);
*)
apply(p_231, d_231, e_231) ==
  CASE VariantTag(e_231) = "Request"
      -> LET (*
        @type: ((Int) => <<Set(Int), Int>>);
      *)
      __QUINT_LAMBDA0(i_217) ==
        IF Cardinality(p_231) < 3
        THEN <<(p_231 \union {i_217}), d_231>>
        ELSE <<p_231, d_231>>
      IN
      __QUINT_LAMBDA0((VariantGetUnsafe("Request", e_231)))
    [] VariantTag(e_231) = "Reply"
      -> LET (*
        @type: (({ id: Int, ok: Bool }) => <<Set(Int), Int>>);
      *)
      __QUINT_LAMBDA1(r_220) ==
        IF r_220["id"] \in p_231 /\ r_220["ok"]
        THEN <<(p_231 \ {r_220["id"]}), ((d_231 + 1) % 4)>>
        ELSE <<p_231, d_231>>
      IN
      __QUINT_LAMBDA1((VariantGetUnsafe("Reply", e_231)))
    [] VariantTag(e_231) = "Failure"
      -> LET (*
        @type: ((Int) => <<Set(Int), Int>>);
      *)
      __QUINT_LAMBDA2(i_223) == <<(p_231 \ {i_223}), d_231>>
      IN
      __QUINT_LAMBDA2((VariantGetUnsafe("Failure", e_231)))
    [] VariantTag(e_231) = "Cancel"
      -> LET (*
        @type: ((Int) => <<Set(Int), Int>>);
      *)
      __QUINT_LAMBDA3(i_226) == <<(p_231 \ {i_226}), d_231>>
      IN
      __QUINT_LAMBDA3((VariantGetUnsafe("Cancel", e_231)))
    [] VariantTag(e_231) = "Reset"
      -> LET (*
        @type: (({ tag: Str }) => <<Set(Int), Int>>);
      *)
      __QUINT_LAMBDA4(id__229) == <<{}, d_231>>
      IN
      __QUINT_LAMBDA4((VariantGetUnsafe("Reset", e_231)))

(*
  @type: ((Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str })) => Str);
*)
label(e_258) ==
  CASE VariantTag(e_258) = "Request"
      -> LET (*
        @type: ((Int) => Str);
      *)
      __QUINT_LAMBDA5(id__244) == "request"
      IN
      __QUINT_LAMBDA5((VariantGetUnsafe("Request", e_258)))
    [] VariantTag(e_258) = "Reply"
      -> LET (*
        @type: (({ id: Int, ok: Bool }) => Str);
      *)
      __QUINT_LAMBDA6(id__247) == "reply"
      IN
      __QUINT_LAMBDA6((VariantGetUnsafe("Reply", e_258)))
    [] VariantTag(e_258) = "Failure"
      -> LET (*
        @type: ((Int) => Str);
      *)
      __QUINT_LAMBDA7(id__250) == "failure"
      IN
      __QUINT_LAMBDA7((VariantGetUnsafe("Failure", e_258)))
    [] VariantTag(e_258) = "Cancel"
      -> LET (*@type: ((Int) => Str); *) __QUINT_LAMBDA8(id__253) == "cancel" IN
      __QUINT_LAMBDA8((VariantGetUnsafe("Cancel", e_258)))
    [] VariantTag(e_258) = "Reset"
      -> LET (*
        @type: (({ tag: Str }) => Str);
      *)
      __QUINT_LAMBDA9(id__256) == "reset"
      IN
      __QUINT_LAMBDA9((VariantGetUnsafe("Reset", e_258)))

(*
  @type: ((Int) => Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str }));
*)
Failure(__FailureParam_26) == Variant("Failure", __FailureParam_26)

(*
  @type: ((Int) => Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str }));
*)
Cancel(__CancelParam_32) == Variant("Cancel", __CancelParam_32)

(*
  @type: (() => Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str }));
*)
Reset == Variant("Reset", [tag |-> "UNIT"])

(*
  @type: (() => Bool);
*)
init == pending = {} /\ done = 0 /\ last = "init"

(*
  @type: (() => Bool);
*)
safety == Cardinality(pending) <= 3 /\ done < 4

(*
  @type: (() => Bool);
*)
broken == done < 3

(*
  @type: ((Str) => Set(Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str })));
*)
eventsWithTag(tag_117) ==
  IF tag_117 = "request"
  THEN { (Request(i_71)): i_71 \in Ids }
  ELSE IF tag_117 = "reply"
  THEN UNION ({
    ({ (Reply([id |-> i_90, ok |-> ok_88])): ok_88 \in { TRUE, FALSE } }):
      i_90 \in Ids
  })
  ELSE IF tag_117 = "failure"
  THEN { (Failure(i_100)): i_100 \in Ids }
  ELSE IF tag_117 = "cancel"
  THEN { (Cancel(i_109)): i_109 \in Ids }
  ELSE {(Reset)}

(*
  @type: ((Set(Int), Bool) => Set(Cancel(Int) | Failure(Int) | Reply({ id: Int, ok: Bool }) | Request(Int) | Reset({ tag: Str })));
*)
ownedEvents(p_147, failing_147) ==
  IF failing_147
  THEN { (Failure(i_131)): i_131 \in p_147 }
  ELSE { (Reply([id |-> i_141, ok |-> TRUE])): i_141 \in p_147 }
    \union {(Reset)}

(*
  @type: (() => Bool);
*)
step ==
  \E preferOwned \in { TRUE, FALSE }:
    \E preferFailure \in { TRUE, FALSE }:
      \E tag \in Tags:
        \E e \in IF preferOwned
        THEN ownedEvents(pending, (preferFailure /\ pending /= {}))
        ELSE eventsWithTag(tag):
          LET (*
            @type: (() => <<Set(Int), Int>>);
          *)
          after == apply(pending, done, e)
          IN
          preferOwned = preferOwned
            /\ preferFailure = preferFailure
            /\ tag = tag
            /\ pending' := (after)[1]
            /\ done' := (after)[2]
            /\ last' := label(e)

(*
  @type: (() => Bool);
*)
q_init == init

(*
  @type: (() => Bool);
*)
q_inv == safety

(*
  @type: (() => Bool);
*)
q_step == step

================================================================================
