You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Shared definitions (TS.idr)

```
||| A small embedded DSL for finite transition systems, shared by the five
||| models. A model is a `System cfg st act`:
|||   - `cfg`  constant configuration (relations that do not change),
|||   - `st`   the state (an immutable value),
|||   - `act`  the actions with their arguments (an ADT).
||| `step` is a guarded function: `Nothing` when the guard fails, otherwise
||| the next state. Variables an action does not assign keep their value
||| because the step returns the old record with only some fields replaced.
|||
||| Properties come in two forms that must agree:
|||   - as types (`Invariant`, `Reachable`, `StepInvariant`, `LeadsToUnderWF`),
|||     with proofs left as holes;
|||   - as executable checks over a breadth-first exploration
|||     (`checkInvariant`, `checkReachable`, ...), which distinguish a
|||     complete exploration from a bounded one.
||| Sort sizes are parameters (`Fin n`); a model never fixes them.
module TS

import public Data.Vect
import public Data.Fin
import public Data.List

%default covering

public export
interface Finite a where
  elems : List a

public export
Finite () where
  elems = [()]

public export
Finite Bool where
  elems = [False, True]

public export
{n : Nat} -> Finite (Fin n) where
  elems = toList (Data.Vect.allFins n)

public export
Finite a => Finite (Maybe a) where
  elems = Nothing :: map Just elems

public export
(Finite a, Finite b) => Finite (a, b) where
  elems = [(x, y) | x <- elems, y <- elems]

vects : Finite a => (k : Nat) -> List (Vect k a)
vects Z = [[]]
vects (S k) = [x :: xs | x <- elems, xs <- vects k]

public export
{n : Nat} -> Finite a => Finite (Vect n a) where
  elems = vects n

public export
forAll : Finite a => (a -> Bool) -> Bool
forAll p = all p elems

public export
exists : Finite a => (a -> Bool) -> Bool
exists p = any p elems

public export
implies : Bool -> Lazy Bool -> Bool
implies a b = not a || b

||| `Set n` is the characteristic vector of a subset of `Fin n`.
public export
Set : Nat -> Type
Set n = Vect n Bool

public export
empty : {n : Nat} -> Set n
empty = replicate n False

public export
member : Fin n -> Set n -> Bool
member = index

public export
insert : Fin n -> Set n -> Set n
insert i s = replaceAt i True s

public export
remove : Fin n -> Set n -> Set n
remove i s = replaceAt i False s

public export
size : Set n -> Nat
size s = length (filter id (toList s))

public export
nonEmpty : Set n -> Bool
nonEmpty s = any id s

||| Total function `Fin n -> a`.
public export
Fun : Nat -> Type -> Type
Fun n a = Vect n a

public export
at : Fun n a -> Fin n -> a
at f i = index i f

public export
upd : Fin n -> a -> Fun n a -> Fun n a
upd i v f = replaceAt i v f

public export
record System (cfg : Type) (st : Type) (act : Type) where
  constructor MkSystem
  ||| Assumption on the configuration (a `fact`); configurations that fail
  ||| it are not checked.
  assume  : cfg -> Bool
  initial : cfg -> st
  step    : cfg -> act -> st -> Maybe st

||| `Reach sys k x`: x is reachable from the initial state under config k.
public export
data Reach : System c s a -> c -> s -> Type where
  Start : Reach sys k (initial sys k)
  Next  : Reach sys k x -> (act : a) -> step sys k act x = Just y -> Reach sys k y

||| Reachable in at most d steps.
public export
data ReachIn : System c s a -> Nat -> c -> s -> Type where
  StartIn : ReachIn sys d k (initial sys k)
  NextIn  : ReachIn sys d k x -> (act : a) -> step sys k act x = Just y
         -> ReachIn sys (S d) k y

public export
Invariant : {c, s, a : Type} -> System c s a -> (c -> s -> Bool) -> Type
Invariant sys p =
  (k : c) -> assume sys k = True -> (x : s) -> Reach sys k x -> p k x = True

public export
Reachable : {c, s, a : Type} -> System c s a -> (c -> s -> Bool) -> Type
Reachable sys p =
  (k : c ** (assume sys k = True, (x : s ** (Reach sys k x, p k x = True))))

public export
ReachableIn : {c, s, a : Type} -> System c s a -> Nat -> (c -> s -> Bool) -> Type
ReachableIn sys d p =
  (k : c ** (assume sys k = True, (x : s ** (ReachIn sys d k x, p k x = True))))

public export
Unreachable : {c, s, a : Type} -> System c s a -> (c -> s -> Bool) -> Type
Unreachable sys p = Not (Reachable sys p)

||| A property of steps (a relation between a reachable state and its
||| successor), e.g. "a shipped order stays shipped".
public export
StepInvariant : {c, s, a : Type} -> System c s a -> (c -> s -> s -> Bool) -> Type
StepInvariant sys r =
  (k : c) -> assume sys k = True -> (x, y : s) -> Reach sys k x
  -> (act : a) -> step sys k act x = Just y -> r k x y = True

||| An infinite behavior: a state and a label for every position.
||| `Nothing` is a stuttering step.
public export
Moves : System c s a -> c -> Maybe a -> s -> s -> Type
Moves sys k Nothing x y = y = x
Moves sys k (Just act) x y = step sys k act x = Just y

public export
record Behavior (sys : System c s a) (k : c) where
  constructor MkBehavior
  state : Nat -> s
  label : Nat -> Maybe a
  start : state 0 = initial sys k
  moves : (i : Nat) -> Moves sys k (label i) (state i) (state (S i))

public export
Enabled : System c s a -> c -> a -> s -> Bool
Enabled sys k act x = isJust (step sys k act x)

||| Weak fairness of one action: if it is enabled from i on, it is taken
||| at some j >= i.
public export
WeakFair : {sys : System c s a} -> {k : c} -> Behavior sys k -> a -> Type
WeakFair b act =
  (i : Nat) -> ((j : Nat) -> LTE i j -> Enabled sys k act (state b j) = True)
  -> (j : Nat ** (LTE i j, label b j = Just act))

||| `□(p → ◇q)` on one behavior.
public export
LeadsTo : {sys : System c s a} -> {k : c} -> Behavior sys k
       -> (s -> Bool) -> (s -> Bool) -> Type
LeadsTo b p q =
  (i : Nat) -> p (state b i) = True
  -> (j : Nat ** (LTE i j, q (state b j) = True))

||| Every behavior that is weakly fair on each action in `fair` satisfies
||| `□(p → ◇q)`.
public export
LeadsToUnderWF : {c, s, a : Type} -> System c s a -> (a -> Bool) -> (c -> s -> Bool) -> (c -> s -> Bool) -> Type
LeadsToUnderWF sys fair p q =
  (k : c) -> assume sys k = True -> (b : Behavior sys k)
  -> ((act : a) -> fair act = True -> WeakFair b act)
  -> LeadsTo b (p k) (q k)

||| `Complete` when the frontier became empty within the fuel; `Bounded`
||| otherwise. Levels are the states first seen at depth 0, 1, 2, ...
public export
data Explored s = Complete (List (List s)) | Bounded (List (List s))

public export
levels : Explored s -> List (List s)
levels (Complete l) = l
levels (Bounded l) = l

public export
succs : (Finite a, Eq s) => System c s a -> c -> s -> List (a, s)
succs sys k x = mapMaybe (\act => map (\y => (act, y)) (step sys k act x)) elems

export
explore : (Finite a, Eq s) => Nat -> System c s a -> c -> Explored s
explore fuel sys k = go fuel [initial sys k] [[initial sys k]]
  where
    go : Nat -> List s -> List (List s) -> Explored s
    go f seen acc =
      let frontier = case acc of
                       (l :: _) => l
                       [] => []
          new = nub [y | x <- frontier, (_, y) <- succs sys k x, not (elem y seen)]
      in case new of
           [] => Complete (reverse acc)
           _ => case f of
                  Z => Bounded (reverse acc)
                  S f' => go f' (seen ++ new) (new :: acc)

public export
data Verdict = Holds | Violated | Reached | NotReached | Inconclusive

public export
Show Verdict where
  show Holds = "holds"
  show Violated = "violated"
  show Reached = "reached"
  show NotReached = "unreachable"
  show Inconclusive = "bounded (inconclusive)"

public export
Eq Verdict where
  Holds == Holds = True
  Violated == Violated = True
  Reached == Reached = True
  NotReached == NotReached = True
  Inconclusive == Inconclusive = True
  _ == _ = False

configs : Finite c => System c s a -> List c
configs sys = filter (assume sys) elems

allConfigs : List Verdict -> Verdict
allConfigs vs =
  if elem Violated vs then Violated
  else if elem Inconclusive vs then Inconclusive else Holds

anyConfig : List Verdict -> Verdict
anyConfig vs =
  if elem Reached vs then Reached
  else if elem Inconclusive vs then Inconclusive else NotReached

export
checkInvariant : (Finite c, Finite a, Eq s) => Nat -> System c s a -> (c -> s -> Bool) -> Verdict
checkInvariant fuel sys p = allConfigs (map one (configs sys))
  where
    one : c -> Verdict
    one k = case explore fuel sys k of
      Complete l => if all (p k) (concat l) then Holds else Violated
      Bounded l => if all (p k) (concat l) then Inconclusive else Violated

||| Reachable at all. A miss counts only when the exploration is complete.
export
checkReachable : (Finite c, Finite a, Eq s) => Nat -> System c s a -> (c -> s -> Bool) -> Verdict
checkReachable fuel sys p = anyConfig (map one (configs sys))
  where
    one : c -> Verdict
    one k = case explore fuel sys k of
      Complete l => if any (p k) (concat l) then Reached else NotReached
      Bounded l => if any (p k) (concat l) then Reached else Inconclusive

||| Reachable within `depth` steps. Every state within `depth` steps is
||| seen even when the exploration is bounded, so a miss is definite.
export
checkReachableWithin : (Finite c, Finite a, Eq s) => Nat -> System c s a -> (c -> s -> Bool) -> Verdict
checkReachableWithin depth sys p = anyConfig (map one (configs sys))
  where
    one : c -> Verdict
    one k = if any (p k) (concat (levels (explore depth sys k))) then Reached else NotReached

export
checkStep : (Finite c, Finite a, Eq s) => Nat -> System c s a -> (c -> s -> s -> Bool) -> Verdict
checkStep fuel sys r = allConfigs (map one (configs sys))
  where
    ok : c -> List s -> Bool
    ok k xs = all (\x => all (\(_, y) => r k x y) (succs sys k x)) xs
    one : c -> Verdict
    one k = case explore fuel sys k of
      Complete l => if ok k (concat l) then Holds else Violated
      Bounded l => if ok k (concat l) then Inconclusive else Violated

||| `□(p → ◇q)` under weak fairness of the actions selected by `fair`,
||| stuttering allowed. A violation is a reachable state with p ∧ ¬q from
||| which a ¬q path reaches a fair strongly connected component of the
||| ¬q subgraph (each fair action is disabled somewhere in it or taken
||| inside it). Every state has a stuttering self-loop.
export
checkLeadsTo : (Finite c, Finite a, Eq a, Eq s) => Nat -> System c s a -> (a -> Bool)
            -> (c -> s -> Bool) -> (c -> s -> Bool) -> Verdict
checkLeadsTo fuel sys fair p q = allConfigs (map one (configs sys))
  where
    fairActs : List a
    fairActs = filter fair elems
    edges : c -> s -> List (a, s)
    edges k x = filter (\(_, y) => not (q k y)) (succs sys k x)
    closure : c -> List s -> List s -> List s
    closure k seen [] = seen
    closure k seen (x :: xs) =
      let new = filter (\y => not (elem y seen)) (nub (map snd (edges k x)))
      in closure k (seen ++ new) (xs ++ new)
    reachFrom : c -> s -> List s
    reachFrom k x = closure k [x] [x]
    scc : c -> s -> List s
    scc k z = filter (\w => elem z (reachFrom k w)) (reachFrom k z)
    fairScc : c -> List s -> Bool
    fairScc k comp = all (\act =>
        any (\w => not (Enabled sys k act w)) comp
        || any (\w => any (\(b, y) => b == act && elem y comp) (edges k w)) comp)
      fairActs
    violatedAt : c -> s -> Bool
    violatedAt k x = p k x && not (q k x)
                     && any (\z => fairScc k (scc k z)) (reachFrom k x)
    one : c -> Verdict
    one k = case explore fuel sys k of
      Complete l => if any (violatedAt k) (concat l) then Violated else Holds
      Bounded l => if any (violatedAt k) (concat l) then Violated else Inconclusive

export
report : String -> Verdict -> Verdict -> IO Bool
report name expected got = do
  let ok = expected == got
  putStrLn ((if ok then "MATCH    " else "MISMATCH ") ++ name ++ ": expected "
            ++ show expected ++ ", got " ++ show got)
  pure ok

export
summary : List Bool -> IO ()
summary rs = putStrLn (if all id rs then "all verdicts match SPEC"
                       else "SOME VERDICTS DO NOT MATCH SPEC")

```

## Model permissions

```
module Permissions

import TS

%default covering

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
    , report "control: NoLeak without assumption" Violated
        (checkInvariant 100 ({ assume := \_ => True } sys2) noLeak)
    ]
  summary rs

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
module Bank

import TS

%default covering

||| Amount argument: an Integer restricted, for enumeration only, to -4..4.
||| Workaround: Integer is not finite, so the checked range is a separate
||| sort `Amount`; the state keeps unbounded Integer.
public export
record Amount where
  constructor MkAmount
  value : Integer

public export
Finite Amount where
  elems = map MkAmount [-4 .. 4]

public export
Eq Amount where
  MkAmount x == MkAmount y = x == y

public export
data Act : Nat -> Type where
  Deposit  : Fin a -> Amount -> Act a
  Withdraw : Fin a -> Amount -> Act a

public export
{a : Nat} -> Finite (Act a) where
  elems = [Deposit x n | x <- elems, n <- elems] ++ [Withdraw x n | x <- elems, n <- elems]

public export
State : Nat -> Type
State a = Fun a Integer

stepB : () -> Act a -> State a -> Maybe (State a)
stepB _ (Deposit x (MkAmount n)) b =
  if n > 0 && at b x + n <= 4 then Just (upd x (at b x + n) b) else Nothing
stepB _ (Withdraw x (MkAmount n)) b =
  if n > 0 && at b x >= n then Just (upd x (at b x - n) b) else Nothing

public export
bank : {a : Nat} -> System () (State a) (Act a)
bank = MkSystem (\_ => True) (\_ => replicate a 0) stepB

nonNegative : {a : Nat} -> () -> State a -> Bool
nonNegative _ b = forAll (\x => at b x >= 0)

rich : {a : Nat} -> () -> State a -> Bool
rich _ b = exists (\x => at b x >= 3)

NonNegative : {a : Nat} -> Type
NonNegative = Invariant (bank {a}) nonNegative

nonNegativeProof : {a : Nat} -> NonNegative {a}
nonNegativeProof = ?nonNegativeProof_hole

||| Rich in one step.
Rich : {a : Nat} -> Type
Rich = ReachableIn (bank {a}) 1 rich

export
main : IO ()
main = do
  let sys = bank {a = 2}
  rs <- sequence
    [ report "NonNegative" Holds (checkInvariant 100 sys nonNegative)
    , report "Rich (within 1 step)" Reached (checkReachableWithin 1 sys rich)
    , report "control: NonNegative with weak withdraw" Violated
        (checkInvariant 3 (MkSystem (\_ => True) (\_ => [0, 0]) weak) nonNegative)
    ]
  summary rs
  where
    weak : () -> Act 2 -> State 2 -> Maybe (State 2)
    weak _ (Withdraw x (MkAmount n)) b = if n > 0 then Just (upd x (at b x - n) b) else Nothing
    weak k act b = stepB k act b

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
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

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
module Sessions

import TS

%default covering

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

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
module Service

import TS

%default covering

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

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
