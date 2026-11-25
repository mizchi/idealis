(* permissions: fixtures/rmodel/permissions.model *)
open Model

type state = {
  grants : (int * int) list;   (* User -> set Role, configuration *)
  reads : (int * int) list;    (* Role -> set Doc, configuration *)
  secret : int list;           (* set Doc, configuration *)
  cleared : int list;          (* set User, configuration *)
  opened : (int * int) list;   (* User -> set Doc, variable ([open] is a keyword) *)
}

let model ~users ~roles ~docs =
  let us = elems users and rs = elems roles and ds = elems docs in
  let can_read s u d = exists rs (fun r -> mem (u, r) s.grants && mem (r, d) s.reads) in
  let init =
    List.concat_map (fun grants ->
      List.concat_map (fun reads ->
        List.concat_map (fun secret ->
          List.map (fun cleared -> { grants; reads; secret; cleared; opened = [] })
            (subsets us)) (subsets ds)) (subsets (pairs rs ds))) (subsets (pairs us rs)) in
  let fact s = forall rs (fun r -> forall ds (fun d -> forall us (fun u ->
      not (mem (r, d) s.reads && mem d s.secret && mem (u, r) s.grants) || mem u s.cleared))) in
  let show s =
    let ps l = String.concat "," (List.map (fun (a, b) -> Printf.sprintf "%d%d" a b) l) in
    let is l = String.concat "," (List.map string_of_int l) in
    Printf.sprintf "grants={%s} reads={%s} secret={%s} cleared={%s} open={%s}"
      (ps s.grants) (ps s.reads) (is s.secret) (is s.cleared) (ps s.opened) in
  let system = {
    init; facts = [ fact ]; show;
    actions = [
      action "read" (pairs us ds) (fun (u, d) s ->
        if can_read s u d then Some { s with opened = add (u, d) s.opened } else None);
      action "close" (pairs us ds) (fun (u, d) s ->
        if mem (u, d) s.opened then Some { s with opened = remove (u, d) s.opened } else None);
    ] } in
  let secret_open s u = exists ds (fun d -> mem (u, d) s.opened && mem d s.secret) in
  system, [
    Invariant ("NoLeak", (fun s -> forall us (fun u -> not (secret_open s u) || mem u s.cleared))), Holds;
    Invariant ("OnlyGranted", (fun s -> List.for_all (fun (u, d) -> can_read s u d) s.opened)), Holds;
    Reachable ("SecretRead", (fun s -> exists us (secret_open s))), ReachableAt 1;
    Reachable ("UnclearedSecret", (fun s -> exists us (fun u -> secret_open s u && not (mem u s.cleared)))), Unreachable;
  ]

let () =
  let sys, props = model ~users:2 ~roles:2 ~docs:2 in
  main [ check ~title:"permissions (User=2 Role=2 Doc=2)" sys props ]
