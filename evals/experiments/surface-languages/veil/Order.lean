module

public import Veil

/-! Order (SPEC 3) in Veil. `Status` is a Lean inductive type with payloads. -/

veil module Order

type order

-- Veil's `enum` declares a sort of the module (a type parameter with
-- distinctness axioms), not a Lean type, so it cannot be a constructor
-- field. Carrier and Reason are Lean inductives like Status.
@[veil_decl]
inductive Carrier where
  | post
  | courier
deriving instance Veil.Enumeration for Carrier

@[veil_decl]
inductive Reason where
  | customer
  | fraud
deriving instance Veil.Enumeration for Reason

@[veil_decl]
inductive Status where
  | draft
  | placed
  | shipped (tracking : Carrier)
  | cancelled (reason : Reason)
deriving instance Veil.Enumeration for Status

function status : order → Status

#gen_state

after_init {
  status O := .draft
}

-- Payloads are read by equality with a constructor (`status o = .shipped c`
-- with `c` bound), which is total. Workaround for `match`: it runs in the
-- explicit-state checkers but not in the SMT ones. A `match` inline in
-- `require` has no `Decidable` instance; a Lean `def` by `match` is treated
-- as opaque by SMT (a `sat trace` then "found" `cancel` of a shipped order);
-- unfolding it with `@[smtSimp]` fails with "SMT-LIB does not support
-- lambdas" (the match motive).

action place (o : order) {
  require status o = .draft
  status o := .placed
}

action ship (o : order) (c : Carrier) {
  require status o = .placed
  status o := .shipped c
}

action cancel (o : order) (r : Reason) {
  require status o = .draft ∨ status o = .placed
  status o := .cancelled r
}

-- Every order ends shipped or cancelled; such a state has no successor and
-- is not a deadlock.
termination ∀ o, (∃ c, status o = .shipped c) ∨ (∃ r, status o = .cancelled r)

-- ShippedStays is a property of a step, not of a state. Direct form: a
-- temporal formula with `◯` (next), checked by `#temporal_check`.
temporal [shipped_stays]
  ∀ o : order, ∀ c : Carrier, □ (⌜ status o = .shipped c ⌝ → ◯ ⌜ status o = .shipped c ⌝)

#gen_spec
#gen_executable

#model_check { order := Fin 2 } {}

#temporal_check { order := Fin 2 } {}

-- FraudCancelled: reachable in one step.
sat trace [fraud_cancelled] {
  cancel
  assert (∃ o, status o = .cancelled .fraud)
}

-- ShippedCancelled is not expressible as a reachable state: one value of
-- `Status` has one constructor. Lean proves it for the type, outside the model.
theorem shipped_not_cancelled (c : Carrier) (r : Reason) :
    Status.shipped c ≠ Status.cancelled r := by
  intro h; cases h

end Order
