(* sessions: fixtures/rmodel/sessions.model *)
open Model

type state = {
  admins : int list;        (* set User, configuration *)
  active : int list;        (* set User *)
  locked : int option;      (* lone User *)
}

let model ~users =
  let us = elems users in
  let show s =
    let is l = String.concat "," (List.map string_of_int l) in
    Printf.sprintf "admins={%s} active={%s} locked=%s" (is s.admins) (is s.active)
      (match s.locked with None -> "-" | Some u -> string_of_int u) in
  let system = {
    init = List.map (fun admins -> { admins; active = []; locked = None }) (subsets us);
    facts = [ (fun s -> s.admins <> []) ]; show;
    actions = [
      action "login" us (fun u s ->
        if not (mem u s.active) && List.length s.active < 2 then Some { s with active = add u s.active } else None);
      action "logout" us (fun u s ->
        if mem u s.active then Some { s with active = remove u s.active } else None);
      action "lock" us (fun u s ->
        if s.locked = None && mem u s.admins && mem u s.active then Some { s with locked = Some u } else None);
      action "unlock" us (fun u s ->
        if s.locked = Some u then Some { s with locked = None } else None);
    ] } in
  let locked_in set s = match s.locked with None -> true | Some u -> mem u set in
  system, [
    Invariant ("Capacity", (fun s -> List.length s.active <= 2)), Holds;
    Invariant ("LockedByAdmin", (fun s -> locked_in s.admins s)), Holds;
    Invariant ("LockedIsActive", (fun s -> locked_in s.active s)), Violated;
    Reachable ("Full", (fun s -> List.length s.active = 2 && s.locked <> None)), ReachableAt 3;
    Reachable ("ThreeActive", (fun s -> List.length s.active = 3)), Unreachable;
  ]

let () = let sys, props = model ~users:3 in main [ check ~title:"sessions (User=3)" sys props ]
