(* A small embedded DSL for finite transition systems, and a breadth-first
   explicit-state checker. Standard library only.

   - A sort is a size n; its elements are the ints 0 .. n-1. The sizes are
     arguments of the model function, given where the model is checked.
   - A state is any immutable OCaml value compared structurally (records of
     ints, sorted lists, options, variants). Sets and relations are sorted,
     duplicate-free lists so that equal sets have one representation.
   - An action is a name and a list of instances, one per parameter tuple;
     each instance is a guarded function [state -> state option].
   - Facts are filters on the list of initial states: [init] lists every
     configuration, and the checker keeps those that satisfy the facts.
   - Every state has a stuttering self-loop (TLA+ behaviours may stutter). *)

(* ---- finite sorts, sets, relations ---- *)

let elems n = List.init n Fun.id
let pairs xs ys = List.concat_map (fun x -> List.map (fun y -> (x, y)) ys) xs
let rec subsets = function
  | [] -> [ [] ]
  | x :: rest ->
      let r = subsets rest in
      r @ List.map (fun s -> x :: s) r

(* sets as sorted lists *)
let mem = List.mem
let add x s = List.sort_uniq compare (x :: s)
let remove x s = List.filter (fun y -> y <> x) s
let inter a b = List.filter (fun x -> List.mem x b) a
let forall xs p = List.for_all p xs
let exists xs p = List.exists p xs

(* total functions over a sort as immutable lists indexed by element *)
let get f i = List.nth f i
let set f i v = List.mapi (fun j x -> if j = i then v else x) f
let const n v = List.init n (fun _ -> v)
(* all functions from a sort of size n to the values vs *)
let rec functions n vs =
  if n = 0 then [ [] ]
  else List.concat_map (fun f -> List.map (fun v -> v :: f) vs) (functions (n - 1) vs)

(* ---- systems and properties ---- *)

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
      (* [] (p -> <> q) under weak fairness of each named action
         (WF of the disjunction of its instances). *)

type expect =
  | Holds
  | Violated
  | ReachableAt of int      (* reachable, shortest witness has this depth *)
  | Unreachable

(* ---- checker ---- *)

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

(* Fair-lasso search for [] (p -> <> q). A violation is a reachable state with
   p /\ ~q, followed by an infinite fair path through ~q states only. Restrict
   the graph to ~q states reachable from such a state, compute SCCs (Tarjan;
   every node has a stuttering loop, so every SCC is cyclic). An SCC C carries
   a fair infinite path (visiting all of C forever) iff for every fair action
   A, some edge inside C is an A step, or A is disabled at some state of C.
   Any fair run settles in one SCC, so this is exact for weak fairness. *)
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
      (* a root that reaches the component, for the report *)
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
