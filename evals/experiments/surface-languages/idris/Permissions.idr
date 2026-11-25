module Permissions

import TS

%default covering

-- Sort sizes are parameters: User = Fin u, Role = Fin r, Doc = Fin d.

public export
record Config (u, r, d : Nat) where
  constructor MkConfig
  grants  : Fun u (Set r)
  reads   : Fun r (Set d)
  secret  : Set d
  cleared : Set u

public export
{u, r, d : Nat} -> Finite (Config u r d) where
  elems = [MkConfig g rd s c | g <- elems, rd <- elems, s <- elems, c <- elems]

||| open: User -> set Doc
public export
State : (u, d : Nat) -> Type
State u d = Fun u (Set d)

public export
data Act : (u, d : Nat) -> Type where
  Read  : Fin u -> Fin d -> Act u d
  Close : Fin u -> Fin d -> Act u d

public export
{u, d : Nat} -> Finite (Act u d) where
  elems = [Read x y | x <- elems, y <- elems] ++ [Close x y | x <- elems, y <- elems]

public export
Eq (Act u d) where
  Read a b == Read c e = a == c && b == e
  Close a b == Close c e = a == c && b == e
  _ == _ = False

canRead : {r : Nat} -> Config u r d -> Fin u -> Fin d -> Bool
canRead k x y = exists (\ro => member ro (at k.grants x) && member y (at k.reads ro))

||| Assumption: a role that reads a secret document is granted only to
||| cleared users.
assumption : {u, r, d : Nat} -> Config u r d -> Bool
assumption k = forAll (\ro => forAll (\y => forAll (\x =>
  implies (member y (at k.reads ro) && member y k.secret && member ro (at k.grants x))
          (member x k.cleared))))

stepP : {u, r, d : Nat} -> Config u r d -> Act u d -> State u d -> Maybe (State u d)
stepP k (Read x y) o =
  if canRead k x y then Just (upd x (insert y (at o x)) o) else Nothing
stepP k (Close x y) o =
  if member y (at o x) then Just (upd x (remove y (at o x)) o) else Nothing

public export
permissions : {u, r, d : Nat} -> System (Config u r d) (State u d) (Act u d)
permissions = MkSystem assumption (\_ => replicate u empty) stepP

-- Properties --------------------------------------------------------

noLeak : {u, r, d : Nat} -> Config u r d -> State u d -> Bool
noLeak k o = forAll (\x => forAll (\y =>
  implies (member y (at o x) && member y k.secret) (member x k.cleared)))

onlyGranted : {u, r, d : Nat} -> Config u r d -> State u d -> Bool
onlyGranted k o = forAll (\x => forAll (\y => implies (member y (at o x)) (canRead k x y)))

secretRead : {u, r, d : Nat} -> Config u r d -> State u d -> Bool
secretRead k o = exists (\x => exists (\y => member y (at o x) && member y k.secret))

unclearedSecret : {u, r, d : Nat} -> Config u r d -> State u d -> Bool
unclearedSecret k o = exists (\x => exists (\y =>
  member y (at o x) && member y k.secret && not (member x k.cleared)))

-- As types, for every size (proofs left as holes).
NoLeak : {u, r, d : Nat} -> Type
NoLeak = Invariant (permissions {u} {r} {d}) noLeak

noLeakProof : {u, r, d : Nat} -> NoLeak {u} {r} {d}
noLeakProof = ?noLeakProof_hole

OnlyGranted : {u, r, d : Nat} -> Type
OnlyGranted = Invariant (permissions {u} {r} {d}) onlyGranted

SecretRead : {u, r, d : Nat} -> Type
SecretRead = Reachable (permissions {u} {r} {d}) secretRead

UnclearedSecret : {u, r, d : Nat} -> Type
UnclearedSecret = Unreachable (permissions {u} {r} {d}) unclearedSecret

-- Execution with User = Role = Doc = 2 ---------------------------------

sys2 : System (Config 2 2 2) (State 2 2) (Act 2 2)
sys2 = permissions

export
main : IO ()
main = do
  rs <- sequence
    [ report "NoLeak" Holds (checkInvariant 100 sys2 noLeak)
    , report "OnlyGranted" Holds (checkInvariant 100 sys2 onlyGranted)
    , report "SecretRead" Reached (checkReachable 100 sys2 secretRead)
    , report "UnclearedSecret" NotReached (checkReachable 100 sys2 unclearedSecret)
    -- Control: without the assumption on the configuration, NoLeak fails.
    , report "control: NoLeak without assumption" Violated
        (checkInvariant 100 ({ assume := \_ => True } sys2) noLeak)
    ]
  summary rs
