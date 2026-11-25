module Sessions

import TS

%default covering

-- User = Fin n. Config: admins (assumed non-empty).

public export
record State (n : Nat) where
  constructor MkState
  active : Set n
  ||| lone User: Maybe makes "0 or 1" a type, not a check.
  locked : Maybe (Fin n)

public export
Eq (State n) where
  MkState a l == MkState a' l' = a == a' && l == l'

public export
data Act : Nat -> Type where
  Login  : Fin n -> Act n
  Logout : Fin n -> Act n
  Lock   : Fin n -> Act n
  Unlock : Fin n -> Act n

public export
{n : Nat} -> Finite (Act n) where
  elems = map Login elems ++ map Logout elems ++ map Lock elems ++ map Unlock elems

stepS : Set n -> Act n -> State n -> Maybe (State n)
stepS _ (Login u) s =
  if not (member u s.active) && size s.active < 2
    then Just ({ active := insert u s.active } s) else Nothing
stepS _ (Logout u) s =
  if member u s.active then Just ({ active := remove u s.active } s) else Nothing
stepS admins (Lock u) s = case s.locked of
  Nothing => if member u admins && member u s.active
               then Just ({ locked := Just u } s) else Nothing
  Just _ => Nothing
stepS _ (Unlock u) s = case s.locked of
  Just v => if v == u then Just ({ locked := Nothing } s) else Nothing
  Nothing => Nothing

public export
sessions : {n : Nat} -> System (Set n) (State n) (Act n)
sessions = MkSystem nonEmpty (\_ => MkState empty Nothing) stepS

capacity : Set n -> State n -> Bool
capacity _ s = size s.active <= 2

lockedByAdmin : Set n -> State n -> Bool
lockedByAdmin admins s = case s.locked of
  Just u => member u admins
  Nothing => True

lockedIsActive : Set n -> State n -> Bool
lockedIsActive _ s = case s.locked of
  Just u => member u s.active
  Nothing => True

full : Set n -> State n -> Bool
full _ s = size s.active == 2 && isJust s.locked

threeActive : Set n -> State n -> Bool
threeActive _ s = size s.active == 3

Capacity : {n : Nat} -> Type
Capacity = Invariant (sessions {n}) capacity

LockedByAdmin : {n : Nat} -> Type
LockedByAdmin = Invariant (sessions {n}) lockedByAdmin

||| Does not hold: lock, then logout. Stated as the negation of the
||| invariant; its proof would be a counterexample trace.
LockedIsActiveFails : {n : Nat} -> Type
LockedIsActiveFails = Not (Invariant (sessions {n}) lockedIsActive)

Full : {n : Nat} -> Type
Full = Reachable (sessions {n}) full

ThreeActive : {n : Nat} -> Type
ThreeActive = Unreachable (sessions {n}) threeActive

export
main : IO ()
main = do
  let sys = sessions {n = 3}
  rs <- sequence
    [ report "Capacity" Holds (checkInvariant 100 sys capacity)
    , report "LockedByAdmin" Holds (checkInvariant 100 sys lockedByAdmin)
    , report "LockedIsActive" Violated (checkInvariant 100 sys lockedIsActive)
    , report "Full" Reached (checkReachable 100 sys full)
    , report "ThreeActive" NotReached (checkReachable 100 sys threeActive)
    ]
  summary rs
