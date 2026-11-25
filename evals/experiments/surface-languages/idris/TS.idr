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

------------------------------------------------------------------------
-- Finite sorts

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

------------------------------------------------------------------------
-- Sets and total functions over a sort of size n

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

------------------------------------------------------------------------
-- Transition systems

public export
record System (cfg : Type) (st : Type) (act : Type) where
  constructor MkSystem
  ||| Assumption on the configuration (a `fact`); configurations that fail
  ||| it are not checked.
  assume  : cfg -> Bool
  initial : cfg -> st
  step    : cfg -> act -> st -> Maybe st

------------------------------------------------------------------------
-- Properties as types

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

------------------------------------------------------------------------
-- Execution: breadth-first exploration

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

-- Combine per-config verdicts: a violation anywhere is final; Holds
-- needs every config complete.
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

------------------------------------------------------------------------
-- Reporting

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
