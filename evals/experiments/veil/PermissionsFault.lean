module

public import Veil

/-! fixtures/rmodel/permissions.model written by hand in Veil, to compare
the two semantics (the Veil experiment after plan 0105). `rel` becomes
`immutable relation`, `fact` an `assumption`, `var` a `relation`, `init`
the `after_init` block, and each action a Veil `action` with `require`. -/

veil module PermissionsFault

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
  -- fault: no grant check
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

sat trace [secret_read] {
  read
  assert (∃ u d, opened u d ∧ secret d)
}

end PermissionsFault
