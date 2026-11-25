(* order: algebraic status *)
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
  (* No wildcard patterns: adding a constructor to [status] makes every match
     above and below non-exhaustive (warning 8, an error in run.sh). *)
  let stays before after = match before, after with
    | Shipped c, Shipped c' -> c = c'
    | Shipped _, (Draft | Placed | Cancelled _) -> false
    | (Draft | Placed | Cancelled _), _ -> true in
  system, [
    Step ("ShippedStays", (fun s s' -> List.for_all2 stays s.status s'.status)), Holds;
    Reachable ("FraudCancelled", (fun s -> List.exists (function
      | Cancelled Fraud -> true
      | Cancelled Customer | Draft | Placed | Shipped _ -> false) s.status)), ReachableAt 1;
    (* ShippedCancelled: not expressible as a state. A [status] value is one
       constructor, so "Shipped and Cancelled" cannot be built; the type rules
       it out, not the exploration. The predicate itself still type-checks
       (two matches joined by &&) and is false by construction, so checking it
       would only restate the type. *)
    Reachable ("ShippedCancelled", (fun s -> List.exists (fun st ->
      (match st with Shipped _ -> true | Draft | Placed | Cancelled _ -> false)
      && (match st with Cancelled _ -> true | Draft | Placed | Shipped _ -> false)) s.status)), Unreachable;
  ]

let () = let sys, props = model ~orders:2 in main [ check ~title:"order (Order=2)" sys props ]
