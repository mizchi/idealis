module

public import Veil

/-! Bank (SPEC 2) in Veil. Balances are Lean `Int` (mathematical integers,
also in SMT). -/

veil module Bank

type account

-- Workaround: the explicit-state checker enumerates action arguments, and
-- `Int` has no finite enumeration (`FinEncodableInjOnly Int` is missing), so
-- `action deposit (a : account) (n : Int)` elaborates and verifies but cannot
-- be model checked. The amount is instead an uninterpreted sort `amount`
-- with a fixed map `value : amount → Int`. SMT checks every interpretation
-- (so every Int amount); `#model_check` takes `amount := Fin 9` and
-- `value k = k - 4`, the SPEC range -4..4, as check arguments.
-- (Tried and rejected: `let k : Nat :| k ∈ List.range 9` model checks but SMT
-- fails on `Membership.mem`; a `(k : Fin 9)` argument verifies but the SMT
-- trace prints a `k` that does not match the step.)
type amount
immutable function value : amount → Int

function balance : account → Int

#gen_state

after_init {
  balance A := 0
}

action deposit (a : account) (k : amount) {
  require value k > 0
  require balance a + value k ≤ 4
  balance a := balance a + value k
}

action withdraw (a : account) (k : amount) {
  require value k > 0
  require balance a ≥ value k
  balance a := balance a - value k
}

safety [non_negative] balance A ≥ 0

#gen_spec

#model_check { account := Fin 2, amount := Fin 9 }
  { value := fun k => (k.val : Int) - 4 }

#check_invariants

-- Rich: one deposit reaches a balance of 3 or more.
sat trace [rich] {
  deposit
  assert (∃ a, balance a ≥ 3)
}

end Bank
