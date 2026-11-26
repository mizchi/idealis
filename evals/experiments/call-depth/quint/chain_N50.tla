--------------------------------- MODULE chain ---------------------------------

EXTENDS Integers, Sequences, FiniteSets, TLC, Apalache, Variants

VARIABLE
  (*
    @type: Int;
  *)
  counter

(*
  @type: ((Int) => Int);
*)
hop0(x_9) == x_9 + 1

(*
  @type: (() => Bool);
*)
init == counter = 0

(*
  @type: (() => Bool);
*)
inv == counter >= 0

(*
  @type: ((Int) => Int);
*)
hop1(x_18) == hop0(x_18) + 1

(*
  @type: ((Int) => Int);
*)
hop2(x_27) == hop1(x_27) + 1

(*
  @type: (() => Bool);
*)
q_init == init

(*
  @type: (() => Bool);
*)
q_inv == inv

(*
  @type: ((Int) => Int);
*)
hop3(x_36) == hop2(x_36) + 1

(*
  @type: ((Int) => Int);
*)
hop4(x_45) == hop3(x_45) + 1

(*
  @type: ((Int) => Int);
*)
hop5(x_54) == hop4(x_54) + 1

(*
  @type: ((Int) => Int);
*)
hop6(x_63) == hop5(x_63) + 1

(*
  @type: ((Int) => Int);
*)
hop7(x_72) == hop6(x_72) + 1

(*
  @type: ((Int) => Int);
*)
hop8(x_81) == hop7(x_81) + 1

(*
  @type: ((Int) => Int);
*)
hop9(x_90) == hop8(x_90) + 1

(*
  @type: ((Int) => Int);
*)
hop10(x_99) == hop9(x_99) + 1

(*
  @type: ((Int) => Int);
*)
hop11(x_108) == hop10(x_108) + 1

(*
  @type: ((Int) => Int);
*)
hop12(x_117) == hop11(x_117) + 1

(*
  @type: ((Int) => Int);
*)
hop13(x_126) == hop12(x_126) + 1

(*
  @type: ((Int) => Int);
*)
hop14(x_135) == hop13(x_135) + 1

(*
  @type: ((Int) => Int);
*)
hop15(x_144) == hop14(x_144) + 1

(*
  @type: ((Int) => Int);
*)
hop16(x_153) == hop15(x_153) + 1

(*
  @type: ((Int) => Int);
*)
hop17(x_162) == hop16(x_162) + 1

(*
  @type: ((Int) => Int);
*)
hop18(x_171) == hop17(x_171) + 1

(*
  @type: ((Int) => Int);
*)
hop19(x_180) == hop18(x_180) + 1

(*
  @type: ((Int) => Int);
*)
hop20(x_189) == hop19(x_189) + 1

(*
  @type: ((Int) => Int);
*)
hop21(x_198) == hop20(x_198) + 1

(*
  @type: ((Int) => Int);
*)
hop22(x_207) == hop21(x_207) + 1

(*
  @type: ((Int) => Int);
*)
hop23(x_216) == hop22(x_216) + 1

(*
  @type: ((Int) => Int);
*)
hop24(x_225) == hop23(x_225) + 1

(*
  @type: ((Int) => Int);
*)
hop25(x_234) == hop24(x_234) + 1

(*
  @type: ((Int) => Int);
*)
hop26(x_243) == hop25(x_243) + 1

(*
  @type: ((Int) => Int);
*)
hop27(x_252) == hop26(x_252) + 1

(*
  @type: ((Int) => Int);
*)
hop28(x_261) == hop27(x_261) + 1

(*
  @type: ((Int) => Int);
*)
hop29(x_270) == hop28(x_270) + 1

(*
  @type: ((Int) => Int);
*)
hop30(x_279) == hop29(x_279) + 1

(*
  @type: ((Int) => Int);
*)
hop31(x_288) == hop30(x_288) + 1

(*
  @type: ((Int) => Int);
*)
hop32(x_297) == hop31(x_297) + 1

(*
  @type: ((Int) => Int);
*)
hop33(x_306) == hop32(x_306) + 1

(*
  @type: ((Int) => Int);
*)
hop34(x_315) == hop33(x_315) + 1

(*
  @type: ((Int) => Int);
*)
hop35(x_324) == hop34(x_324) + 1

(*
  @type: ((Int) => Int);
*)
hop36(x_333) == hop35(x_333) + 1

(*
  @type: ((Int) => Int);
*)
hop37(x_342) == hop36(x_342) + 1

(*
  @type: ((Int) => Int);
*)
hop38(x_351) == hop37(x_351) + 1

(*
  @type: ((Int) => Int);
*)
hop39(x_360) == hop38(x_360) + 1

(*
  @type: ((Int) => Int);
*)
hop40(x_369) == hop39(x_369) + 1

(*
  @type: ((Int) => Int);
*)
hop41(x_378) == hop40(x_378) + 1

(*
  @type: ((Int) => Int);
*)
hop42(x_387) == hop41(x_387) + 1

(*
  @type: ((Int) => Int);
*)
hop43(x_396) == hop42(x_396) + 1

(*
  @type: ((Int) => Int);
*)
hop44(x_405) == hop43(x_405) + 1

(*
  @type: ((Int) => Int);
*)
hop45(x_414) == hop44(x_414) + 1

(*
  @type: ((Int) => Int);
*)
hop46(x_423) == hop45(x_423) + 1

(*
  @type: ((Int) => Int);
*)
hop47(x_432) == hop46(x_432) + 1

(*
  @type: ((Int) => Int);
*)
hop48(x_441) == hop47(x_441) + 1

(*
  @type: ((Int) => Int);
*)
hop49(x_450) == hop48(x_450) + 1

(*
  @type: ((Int) => Int);
*)
hop50(x_459) == hop49(x_459) + 1

(*
  @type: (() => Bool);
*)
step == counter' := hop50(counter) % 5

(*
  @type: (() => Bool);
*)
q_step == step

================================================================================
