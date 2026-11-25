module Bank

import TS

%default covering

-- Account = Fin a. Balances are mathematical integers (Integer); only
-- the action argument n is enumerated, over -4..4.

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
    -- Control: a withdraw guard without `balance >= n` breaks NonNegative.
    -- Its state space is infinite, so it is explored 3 levels; a violation
    -- found in a bounded exploration is still definite.
    , report "control: NonNegative with weak withdraw" Violated
        (checkInvariant 3 (MkSystem (\_ => True) (\_ => [0, 0]) weak) nonNegative)
    ]
  summary rs
  where
    weak : () -> Act 2 -> State 2 -> Maybe (State 2)
    weak _ (Withdraw x (MkAmount n)) b = if n > 0 then Just (upd x (at b x - n) b) else Nothing
    weak k act b = stepB k act b
