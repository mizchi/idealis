module

public import Veil

/-! Sessions (SPEC 4) in Veil. -/

veil module Sessions

type user

immutable relation admins : user → Bool

relation active : user → Bool
-- Workaround: `lone User` would be `individual locked : Option user`, which
-- elaborates and model checks, but the SMT encoding fails on it ("incorrect
-- number of universe levels Option") for every VC. So `locked` is a relation
-- and "at most one" is the invariant `lone_locked` below.
relation locked : user → Bool

#gen_state

assumption [some_admin] ∃ u, admins u

after_init {
  active U := false
  locked U := false
}

-- Workaround: no cardinality on a relation over an uninterpreted sort in the
-- first-order fragment. `#active < 2` is "no two distinct active users".
action login (u : user) {
  require ¬ active u
  require ¬ ∃ v w, v ≠ w ∧ active v ∧ active w
  active u := true
}

action logout (u : user) {
  require active u
  active u := false
}

action lock (u : user) {
  require ∀ v, ¬ locked v
  require admins u ∧ active u
  locked u := true
}

action unlock (u : user) {
  require locked u
  locked u := false
}

-- Workaround (cardinality): `#active <= 2` is "no three distinct active users".
safety [capacity] ¬ ∃ u v w, u ≠ v ∧ v ≠ w ∧ u ≠ w ∧ active u ∧ active v ∧ active w
safety [lone_locked] ∀ u v, locked u ∧ locked v → u = v
safety [locked_by_admin] ∀ u, locked u → admins u
-- Expected to fail: `logout` of the locking admin keeps the lock.
safety [locked_is_active] ∀ u, locked u → active u

#gen_spec

-- One configuration with User = 3: user 0 is the admin.
set_option veil.violationIsError false in
#model_check { user := Fin 3 } { admins := fun u => u == 0 }

set_option veil.violationIsError false in
#check_invariants

-- Full: two active users and a lock (login, login, lock).
sat trace [full] {
  any 3 actions
  assert (∃ u v w, u ≠ v ∧ active u ∧ active v ∧ locked w)
}

-- ThreeActive: no trace of up to 8 actions (SPEC steps = 8).
unsat trace [three_active] {
  any 8 actions
  assert (∃ u v w, u ≠ v ∧ v ≠ w ∧ u ≠ w ∧ active u ∧ active v ∧ active w)
}

end Sessions
