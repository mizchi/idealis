You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Shared definitions (model.ml)

```


let elems n = List.init n Fun.id
let pairs xs ys = List.concat_map (fun x -> List.map (fun y -> (x, y)) ys) xs
let rec subsets = function
  | [] -> [ [] ]
  | x :: rest ->
      let r = subsets rest in
      r @ List.map (fun s -> x :: s) r

let mem = List.mem
let add x s = List.sort_uniq compare (x :: s)
let remove x s = List.filter (fun y -> y <> x) s
let inter a b = List.filter (fun x -> List.mem x b) a
let forall xs p = List.for_all p xs
let exists xs p = List.exists p xs

let get f i = List.nth f i
let set f i v = List.mapi (fun j x -> if j = i then v else x) f
let const n v = List.init n (fun _ -> v)
let rec functions n vs =
  if n = 0 then [ [] ]
  else List.concat_map (fun f -> List.map (fun v -> v :: f) vs) (functions (n - 1) vs)

type 's action = { name : string; steps : ('s -> 's option) list }

let action name params f = { name; steps = List.map f params }

type 's system = {
  init : 's list;          (* candidate initial states, before facts *)
  facts : ('s -> bool) list;
  actions : 's action list;
  show : 's -> string;
}

type 's prop =
  | Invariant of string * ('s -> bool)
  | Reachable of string * ('s -> bool)
  | Step of string * ('s -> 's -> bool)   (* holds on every transition *)
  | LeadsTo of { name : string; p : 's -> bool; q : 's -> bool; fair : string list }

type expect =
  | Holds
  | Violated
  | ReachableAt of int      (* reachable, shortest witness has this depth *)
  | Unreachable

type 's graph = {
  states : 's array;
  parent : int array;           (* -1 for initial *)
  via : string array;           (* action that discovered the state *)
  depth : int array;
  succ : (int * string) list array;  (* without the stuttering loop *)
}

let explore sys =
  let init = List.sort_uniq compare (List.filter (fun s -> List.for_all (fun f -> f s) sys.facts) sys.init) in
  let index = Hashtbl.create 4096 in
  let states = ref [] and parent = ref [] and via = ref [] and depth = ref [] in
  let n = ref 0 in
  let q = Queue.create () in
  let intern s p a d =
    match Hashtbl.find_opt index s with
    | Some i -> i
    | None ->
        let i = !n in
        incr n;
        Hashtbl.add index s i;
        states := s :: !states; parent := p :: !parent; via := a :: !via; depth := d :: !depth;
        Queue.add (i, s, d) q;
        i
  in
  List.iter (fun s -> ignore (intern s (-1) "init" 0)) init;
  let succ = Hashtbl.create 4096 in
  while not (Queue.is_empty q) do
    let i, s, d = Queue.pop q in
    let out =
      List.concat_map
        (fun a ->
          List.filter_map (fun st -> Option.map (fun s' -> (intern s' i a.name (d + 1), a.name)) (st s)) a.steps)
        sys.actions
    in
    Hashtbl.replace succ i (List.sort_uniq compare out)
  done;
  let arr l = Array.of_list (List.rev l) in
  {
    states = arr !states; parent = arr !parent; via = arr !via; depth = arr !depth;
    succ = Array.init !n (fun i -> Hashtbl.find succ i);
  }

let trace sys g i =
  let rec go i acc = if i < 0 then acc else go g.parent.(i) ((g.via.(i), g.states.(i)) :: acc) in
  go i []
  |> List.map (fun (a, s) -> Printf.sprintf "    %-8s %s" a (sys.show s))
  |> String.concat "\n"

let find_state g p =
  let r = ref None in
  Array.iteri (fun i s -> if !r = None && p s then r := Some i) g.states;
  !r   (* states are numbered in BFS order: the first is a shortest witness *)

let enabled sys name s =
  List.exists (fun a -> a.name = name && List.exists (fun st -> st s <> None) a.steps) sys.actions

let leads_to sys g p q fair =
  let n = Array.length g.states in
  let inside = Array.make n false in
  let roots = ref [] in
  let rec mark i = if not inside.(i) && not (q g.states.(i)) then begin
      inside.(i) <- true; List.iter (fun (j, _) -> mark j) g.succ.(i) end in
  Array.iteri (fun i s -> if p s && not (q s) then (roots := i :: !roots; mark i)) g.states;
  let idx = Array.make n (-1) and low = Array.make n 0 and on = Array.make n false in
  let stack = ref [] and counter = ref 0 and found = ref None in
  let rec strong v =
    idx.(v) <- !counter; low.(v) <- !counter; incr counter;
    stack := v :: !stack; on.(v) <- true;
    List.iter (fun (w, _) ->
        if inside.(w) then
          if idx.(w) < 0 then (strong w; low.(v) <- min low.(v) low.(w))
          else if on.(w) then low.(v) <- min low.(v) idx.(w))
      g.succ.(v);
    if low.(v) = idx.(v) then begin
      let rec pop acc = match !stack with
        | w :: rest -> stack := rest; on.(w) <- false; if w = v then w :: acc else pop (w :: acc)
        | [] -> acc in
      let comp = pop [] in
      let fair_for a =
        List.exists (fun i -> List.exists (fun (j, b) -> b = a && List.mem j comp) g.succ.(i)) comp
        || List.exists (fun i -> not (enabled sys a g.states.(i))) comp in
      if !found = None && List.for_all fair_for fair then found := Some comp
    end in
  Array.iteri (fun i b -> if b && idx.(i) < 0 then strong i) inside;
  match !found with
  | None -> None
  | Some comp ->
      let reaches r = let seen = Array.make n false in
        let rec go i = if inside.(i) && not seen.(i) then (seen.(i) <- true; List.iter (fun (j, _) -> go j) g.succ.(i)) in
        go r; List.exists (fun i -> seen.(i)) comp in
      let r = List.find reaches (List.rev !roots) in
      Some (r, comp)

let check ~title sys props =
  Printf.printf "== %s\n" title;
  let g = explore sys in
  let n = Array.length g.states in
  if n = 0 then (print_endline "  invalid: no initial state satisfies the facts"; exit 2);
  Printf.printf "  complete: %d reachable states\n" n;
  let ok = ref true in
  let report name verdict expected ok' detail =
    if not ok' then ok := false;
    Printf.printf "  %-18s %-14s expected %-14s %s\n" name verdict expected (if ok' then "matched" else "MISMATCH");
    Option.iter print_endline detail in
  List.iter
    (fun (prop, exp) ->
      let exp_s = match exp with Holds -> "holds" | Violated -> "violated"
        | ReachableAt d -> Printf.sprintf "reachable@%d" d | Unreachable -> "unreachable" in
      match prop with
      | Invariant (name, f) ->
          (match find_state g (fun s -> not (f s)) with
           | None -> report name "holds" exp_s (exp = Holds) None
           | Some i -> report name "violated" exp_s (exp = Violated) (Some (trace sys g i)))
      | Reachable (name, f) ->
          (match find_state g f with
           | None -> report name "unreachable" exp_s (exp = Unreachable) None
           | Some i ->
               let v = Printf.sprintf "reachable@%d" g.depth.(i) in
               report name v exp_s (exp = ReachableAt g.depth.(i)) (Some (trace sys g i)))
      | Step (name, f) ->
          let bad = ref None in
          Array.iteri (fun i s -> List.iter (fun (j, _) ->
              if !bad = None && not (f s g.states.(j)) then bad := Some (i, j)) ((i, "stutter") :: g.succ.(i)))
            g.states;
          (match !bad with
           | None -> report name "holds" exp_s (exp = Holds) None
           | Some (i, j) -> report name "violated" exp_s (exp = Violated)
               (Some (trace sys g i ^ "\n    next     " ^ sys.show g.states.(j))))
      | LeadsTo { name; p; q; fair } ->
          (match leads_to sys g p q fair with
           | None -> report name "holds" exp_s (exp = Holds) None
           | Some (r, comp) ->
               let cyc = List.map (fun i -> "    cycle    " ^ sys.show g.states.(i)) comp in
               report name "violated" exp_s (exp = Violated)
                 (Some (trace sys g r ^ "\n" ^ String.concat "\n" cyc))))
    props;
  !ok

let main results = if List.for_all Fun.id results then exit 0 else exit 1

```

## Model permissions

```
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

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
open Model

type state = { balance : int list }   (* Account -> Int *)

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

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
open Model

type carrier = Post | Courier
type reason = Customer | Fraud
type status = Draft | Placed | Shipped of carrier | Cancelled of reason

type state = { status : status list }   (* Order -> Status *)

let carriers = [ Post; Courier ] and reasons = [ Customer; Fraud ]

let show_status = function
  | Draft -> "Draft" | Placed -> "Placed"
  | Shipped Post -> "Shipped(Post)" | Shipped Courier -> "Shipped(Courier)"
  | Cancelled Customer -> "Cancelled(Customer)" | Cancelled Fraud -> "Cancelled(Fraud)"

let model ~orders =
  let os = elems orders in
  let update o f s =
    Option.map (fun v -> { status = set s.status o v }) (f (get s.status o)) in
  let system = {
    init = [ { status = const orders Draft } ]; facts = [];
    show = (fun s -> "[" ^ String.concat "; " (List.map show_status s.status) ^ "]");
    actions = [
      action "place" os (fun o -> update o (function
        | Draft -> Some Placed
        | Placed | Shipped _ | Cancelled _ -> None));
      action "ship" (pairs os carriers) (fun (o, c) -> update o (function
        | Placed -> Some (Shipped c)
        | Draft | Shipped _ | Cancelled _ -> None));
      action "cancel" (pairs os reasons) (fun (o, r) -> update o (function
        | Draft | Placed -> Some (Cancelled r)
        | Shipped _ | Cancelled _ -> None));
    ] } in
  let stays before after = match before, after with
    | Shipped c, Shipped c' -> c = c'
    | Shipped _, (Draft | Placed | Cancelled _) -> false
    | (Draft | Placed | Cancelled _), _ -> true in
  system, [
    Step ("ShippedStays", (fun s s' -> List.for_all2 stays s.status s'.status)), Holds;
    Reachable ("FraudCancelled", (fun s -> List.exists (function
      | Cancelled Fraud -> true
      | Cancelled Customer | Draft | Placed | Shipped _ -> false) s.status)), ReachableAt 1;
    Reachable ("ShippedCancelled", (fun s -> List.exists (fun st ->
      (match st with Shipped _ -> true | Draft | Placed | Cancelled _ -> false)
      && (match st with Cancelled _ -> true | Draft | Placed | Shipped _ -> false)) s.status)), Unreachable;
  ]

let () = let sys, props = model ~orders:2 in main [ check ~title:"order (Order=2)" sys props ]

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
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

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
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

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
