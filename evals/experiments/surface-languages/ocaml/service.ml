(* service: liveness under weak fairness *)
open Model

type state = { waiting : int list; served : int list }   (* set Client each *)

let model ~clients ~fair =
  let cs = elems clients in
  let is l = String.concat "," (List.map string_of_int l) in
  let system = {
    init = [ { waiting = []; served = [] } ]; facts = [];
    show = (fun s -> Printf.sprintf "waiting={%s} served={%s}" (is s.waiting) (is s.served));
    actions = [
      action "request" cs (fun c s ->
        if not (mem c s.waiting || mem c s.served) then Some { s with waiting = add c s.waiting } else None);
      action "serve" cs (fun c s ->
        if mem c s.waiting then Some { waiting = remove c s.waiting; served = add c s.served } else None);
    ] } in
  let expected = if fair = [] then Violated else Holds in
  (* EventuallyServed, one LeadsTo per client: [] (c in waiting -> <> c in served).
     Fairness is WF(serve) for the action as a whole (WF of the disjunction of
     serve(c) over c), checked by the fair-SCC search in Model.leads_to. *)
  system,
  List.map (fun c ->
      LeadsTo { name = Printf.sprintf "EventuallyServed[%d]" c;
                p = (fun s -> mem c s.waiting); q = (fun s -> mem c s.served); fair }, expected) cs
  @ [ Invariant ("Disjoint", (fun s -> inter s.waiting s.served = [])), Holds ]

let () =
  let with_wf, p1 = model ~clients:2 ~fair:[ "serve" ] in
  let without, p2 = model ~clients:2 ~fair:[] in
  let r1 = check ~title:"service (Client=2, WF(serve))" with_wf p1 in
  let r2 = check ~title:"service (Client=2, no fairness)" without p2 in
  main [ r1; r2 ]
