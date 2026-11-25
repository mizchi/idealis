module Order

import TS

%default covering

public export
data Carrier = Post | Courier

public export
data Reason = Customer | Fraud

||| Payloads live inside the constructors: a Shipped order has exactly one
||| tracking, a Cancelled order exactly one reason, other statuses none.
public export
data Status = Draft | Placed | Shipped Carrier | Cancelled Reason

public export
Finite Carrier where elems = [Post, Courier]
public export
Finite Reason where elems = [Customer, Fraud]
public export
Finite Status where
  elems = [Draft, Placed] ++ map Shipped elems ++ map Cancelled elems

public export
Eq Carrier where
  Post == Post = True
  Courier == Courier = True
  _ == _ = False

public export
Eq Reason where
  Customer == Customer = True
  Fraud == Fraud = True
  _ == _ = False

public export
Eq Status where
  Draft == Draft = True
  Placed == Placed = True
  Shipped c == Shipped c' = c == c'
  Cancelled r == Cancelled r' = r == r'
  _ == _ = False

public export
data Act : Nat -> Type where
  Place  : Fin n -> Act n
  Ship   : Fin n -> Carrier -> Act n
  Cancel : Fin n -> Reason -> Act n

public export
{n : Nat} -> Finite (Act n) where
  elems = map Place elems ++ [Ship o c | o <- elems, c <- elems]
       ++ [Cancel o r | o <- elems, r <- elems]

public export
State : Nat -> Type
State n = Fun n Status

-- Guards are pattern matches on the constructor; no partial accessor.
stepO : () -> Act n -> State n -> Maybe (State n)
stepO _ (Place o) s = case at s o of
  Draft => Just (upd o Placed s)
  _ => Nothing
stepO _ (Ship o c) s = case at s o of
  Placed => Just (upd o (Shipped c) s)
  _ => Nothing
stepO _ (Cancel o r) s = case at s o of
  Draft => Just (upd o (Cancelled r) s)
  Placed => Just (upd o (Cancelled r) s)
  _ => Nothing

public export
order : {n : Nat} -> System () (State n) (Act n)
order = MkSystem (\_ => True) (\_ => replicate n Draft) stepO

||| ShippedStays: a property of steps. The tracking is taken out with a
||| total match.
shippedStays : {n : Nat} -> () -> State n -> State n -> Bool
shippedStays _ s s' = forAll (\o => case at s o of
  Shipped c => case at s' o of
    Shipped c' => c == c'
    _ => False
  _ => True)

fraudCancelled : {n : Nat} -> () -> State n -> Bool
fraudCancelled _ s = exists (\o => case at s o of
  Cancelled Fraud => True
  _ => False)

ShippedStays : {n : Nat} -> Type
ShippedStays = StepInvariant (order {n}) shippedStays

shippedStaysProof : {n : Nat} -> ShippedStays {n}
shippedStaysProof = ?shippedStaysProof_hole

FraudCancelled : {n : Nat} -> Type
FraudCancelled = Reachable (order {n}) fraudCancelled

||| ShippedCancelled is ruled out by the type of Status: one value cannot
||| be built by both constructors. This is proved, not left as a hole:
||| the two equations make Shipped c = Cancelled r, which unification
||| rejects.
noShippedCancelled : (s : Status) -> s = Shipped c -> s = Cancelled r -> Void
noShippedCancelled _ Refl Refl impossible

||| Lifted to every order of every state, reachable or not.
NoShippedCancelled : {n : Nat} -> Type
NoShippedCancelled = (s : State n) -> (o : Fin n) -> (c : Carrier) -> (r : Reason)
                  -> at s o = Shipped c -> at s o = Cancelled r -> Void

noShippedCancelledAll : {n : Nat} -> NoShippedCancelled {n}
noShippedCancelledAll s o c r p q = noShippedCancelled (at s o) p q

export
main : IO ()
main = do
  let sys = order {n = 2}
  rs <- sequence
    [ report "ShippedStays" Holds (checkStep 100 sys shippedStays)
    , report "FraudCancelled" Reached (checkReachable 100 sys fraudCancelled)
    -- Control: allowing cancel of a Shipped order breaks ShippedStays.
    , report "control: ShippedStays with cancel from Shipped" Violated
        (checkStep 100 (MkSystem (\_ => True) (\_ => [Draft, Draft]) loose) shippedStays)
    ]
  summary rs
  where
    loose : () -> Act 2 -> State 2 -> Maybe (State 2)
    loose _ (Cancel o r) s = case at s o of
      Cancelled _ => Nothing
      _ => Just (upd o (Cancelled r) s)
    loose k act s = stepO k act s
