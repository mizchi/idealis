(* bank: fixtures/rcheck/bank.model *)
open Model

type state = { balance : int list }   (* Account -> Int *)

(* Int is OCaml's 63-bit int, not a mathematical integer; the guards keep
   balances in 0..4, so no overflow is possible here. *)
let model ~accounts ~ints:(lo, hi) =
  let accts = elems accounts and amounts = List.init (hi - lo + 1) (fun i -> lo + i) in
  let show s = "balance=[" ^ String.concat "," (List.map string_of_int s.balance) ^ "]" in
  let system = {
    init = [ { balance = const accounts 0 } ]; facts = []; show;
    actions = [
      action "deposit" (pairs accts amounts) (fun (a, n) s ->
        let b = get s.balance a in
        if n > 0 && b + n <= 4 then Some { balance = set s.balance a (b + n) } else None);
      action "withdraw" (pairs accts amounts) (fun (a, n) s ->
        let b = get s.balance a in
        if n > 0 && b >= n then Some { balance = set s.balance a (b - n) } else None);
    ] } in
  system, [
    Invariant ("NonNegative", (fun s -> List.for_all (fun b -> b >= 0) s.balance)), Holds;
    Reachable ("Rich", (fun s -> List.exists (fun b -> b >= 3) s.balance)), ReachableAt 1;
  ]

let () =
  let sys, props = model ~accounts:2 ~ints:(-4, 4) in
  main [ check ~title:"bank (Account=2, Int=-4..4)" sys props ]
