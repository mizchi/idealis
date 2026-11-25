module

public import Veil

/-! Permissions (SPEC 1) in Veil, from evals/experiments/veil/Permissions.lean.
`rel` becomes `immutable relation`, `fact` an `assumption`, `var` a
`relation`, `init` the `after_init` block, each action an `action` with
`require`. `open` is a Lean keyword, so the state is `opened`. -/

veil module Permissions

type user
type role
type doc

immutable relation grants : user → role → Bool
immutable relation reads : role → doc → Bool
immutable relation secret : doc → Bool
immutable relation cleared : user → Bool

relation opened : user → doc → Bool

#gen_state

-- A role that reads a secret document is granted only to cleared users.
-- (An assumption closes the state declarations, so it follows them.)
assumption [clearance] ∀ (r : role) (d : doc) (u : user),
  reads r d ∧ secret d ∧ grants u r → cleared u

after_init {
  opened U D := false
}

action read (u : user) (d : doc) {
  require ∃ r, grants u r ∧ reads r d
  opened u d := true
}

action close (u : user) (d : doc) {
  require opened u d
  opened u d := false
}

safety [no_leak] opened U D ∧ secret D → cleared U
invariant [only_granted] opened U D → ∃ r, grants U r ∧ reads r D

#gen_spec

-- One configuration: User0 holds Role0, which reads the secret Doc1, and
-- User0 is cleared.
#model_check { user := Fin 2, role := Fin 2, doc := Fin 2 }
  { grants := fun u r => u == 0 && r == 0,
    reads := fun r d => r == 0 && d == 1,
    secret := fun d => d == 1,
    cleared := fun u => u == 0 }

-- Every size and configuration, by SMT.
#check_invariants

-- SecretRead: reachable in one step.
sat trace [secret_read] {
  read
  assert (∃ u d, opened u d ∧ secret d)
}

-- UnclearedSecret: no trace of up to 6 actions (SPEC steps = 6), for every
-- size and configuration. (`unsat trace` is bounded; the unbounded answer is
-- `no_leak` being inductive above.)
unsat trace [uncleared_secret] {
  any 6 actions
  assert (∃ u d, ¬ cleared u ∧ opened u d ∧ secret d)
}

end Permissions
