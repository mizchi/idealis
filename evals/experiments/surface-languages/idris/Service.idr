module Service

import TS

%default covering

-- Client = Fin n.

public export
record State (n : Nat) where
  constructor MkState
  waiting : Set n
  served  : Set n

public export
Eq (State n) where
  MkState w s == MkState w' s' = w == w' && s == s'

public export
data Act : Nat -> Type where
  Request : Fin n -> Act n
  Serve   : Fin n -> Act n

public export
Eq (Act n) where
  Request c == Request c' = c == c'
  Serve c == Serve c' = c == c'
  _ == _ = False

public export
{n : Nat} -> Finite (Act n) where
  elems = map Request elems ++ map Serve elems

stepV : () -> Act n -> State n -> Maybe (State n)
stepV _ (Request c) s =
  if not (member c s.waiting) && not (member c s.served)
    then Just ({ waiting := insert c s.waiting } s) else Nothing
stepV _ (Serve c) s =
  if member c s.waiting
    then Just (MkState (remove c s.waiting) (insert c s.served)) else Nothing

public export
service : {n : Nat} -> System () (State n) (Act n)
service = MkSystem (\_ => True) (\_ => MkState empty empty) stepV

disjoint : {n : Nat} -> () -> State n -> Bool
disjoint _ s = forAll (\c => not (member c s.waiting && member c s.served))

isServe : Act n -> Bool
isServe (Serve _) = True
isServe (Request _) = False

noFairness : Act n -> Bool
noFairness _ = False

Disjoint : {n : Nat} -> Type
Disjoint = Invariant (service {n}) disjoint

||| Weak fairness on each instance serve(c), stated over infinite
||| behaviors (functions Nat -> State with labels; stuttering allowed).
EventuallyServed : {n : Nat} -> Type
EventuallyServed = (c : Fin n) ->
  LeadsToUnderWF (service {n}) isServe
    (\_, s => member c s.waiting) (\_, s => member c s.served)

eventuallyServedProof : {n : Nat} -> EventuallyServed {n}
eventuallyServedProof = ?eventuallyServedProof_hole

||| Without fairness it fails (stutter forever after a request).
NotEventuallyServedUnfair : {n : Nat} -> Type
NotEventuallyServedUnfair = Not ((c : Fin n) ->
  LeadsToUnderWF (service {n}) noFairness
    (\_, s => member c s.waiting) (\_, s => member c s.served))

-- Executable: the DSL's fair-SCC check over the finite graph.
eventuallyServed : (Act 2 -> Bool) -> Verdict
eventuallyServed fair =
  let vs = map (\c => checkLeadsTo 100 (service {n = 2}) fair
                        (\_, s => member c s.waiting) (\_, s => member c s.served))
               (the (List (Fin 2)) elems)
  in if elem Violated vs then Violated
     else if elem Inconclusive vs then Inconclusive else Holds

export
main : IO ()
main = do
  let sys = service {n = 2}
  rs <- sequence
    [ report "Disjoint" Holds (checkInvariant 100 sys disjoint)
    , report "EventuallyServed (WF on serve)" Holds (eventuallyServed isServe)
    , report "EventuallyServed (no fairness)" Violated (eventuallyServed noFairness)
    ]
  summary rs
