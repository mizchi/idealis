You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Model permissions

```
module

public import Veil

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

#model_check { user := Fin 2, role := Fin 2, doc := Fin 2 }
  { grants := fun u r => u == 0 && r == 0,
    reads := fun r d => r == 0 && d == 1,
    secret := fun d => d == 1,
    cleared := fun u => u == 0 }

#check_invariants

sat trace [secret_read] {
  read
  assert (∃ u d, opened u d ∧ secret d)
}

unsat trace [uncleared_secret] {
  any 6 actions
  assert (∃ u d, ¬ cleared u ∧ opened u d ∧ secret d)
}

end Permissions

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
module

public import Veil

veil module Bank

type account

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

sat trace [rich] {
  deposit
  assert (∃ a, balance a ≥ 3)
}

end Bank

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
module

public import Veil

veil module Order

type order

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

termination ∀ o, (∃ c, status o = .shipped c) ∨ (∃ r, status o = .cancelled r)

temporal [shipped_stays]
  ∀ o : order, ∀ c : Carrier, □ (⌜ status o = .shipped c ⌝ → ◯ ⌜ status o = .shipped c ⌝)

#gen_spec
#gen_executable

#model_check { order := Fin 2 } {}

#temporal_check { order := Fin 2 } {}

sat trace [fraud_cancelled] {
  cancel
  assert (∃ o, status o = .cancelled .fraud)
}

theorem shipped_not_cancelled (c : Carrier) (r : Reason) :
    Status.shipped c ≠ Status.cancelled r := by
  intro h; cases h

end Order

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
module

public import Veil

veil module Sessions

type user

immutable relation admins : user → Bool

relation active : user → Bool
relation locked : user → Bool

#gen_state

assumption [some_admin] ∃ u, admins u

after_init {
  active U := false
  locked U := false
}

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

safety [capacity] ¬ ∃ u v w, u ≠ v ∧ v ≠ w ∧ u ≠ w ∧ active u ∧ active v ∧ active w
safety [lone_locked] ∀ u v, locked u ∧ locked v → u = v
safety [locked_by_admin] ∀ u, locked u → admins u
safety [locked_is_active] ∀ u, locked u → active u

#gen_spec

set_option veil.violationIsError false in
#model_check { user := Fin 3 } { admins := fun u => u == 0 }

set_option veil.violationIsError false in
#check_invariants

sat trace [full] {
  any 3 actions
  assert (∃ u v w, u ≠ v ∧ active u ∧ active v ∧ locked w)
}

unsat trace [three_active] {
  any 8 actions
  assert (∃ u v w, u ≠ v ∧ v ≠ w ∧ u ≠ w ∧ active u ∧ active v ∧ active w)
}

end Sessions

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
module

public import Veil

veil module Service

type client

relation waiting : client → Bool
relation served : client → Bool

#gen_state

after_init {
  waiting C := false
  served C := false
}

action request (c : client) {
  require ¬ waiting c ∧ ¬ served c
  waiting c := true
}

action serve (c : client) {
  require waiting c
  waiting c := false
  served c := true
}

action stutter { pure () }

safety [disjoint] ∀ c, ¬ (waiting c ∧ served c)

temporal [eventually_served_fair]
  (∀ c : client, 𝒲ℱ (serve c)) →
    ∀ c : client, □ (⌜ waiting c ⌝ → ◇ ⌜ served c ⌝)

temporal [eventually_served]
  ∀ c : client, □ (⌜ waiting c ⌝ → ◇ ⌜ served c ⌝)

#gen_spec
#gen_executable

#model_check { client := Fin 2 } {}

#check_invariants

set_option veil.violationIsError false in
#temporal_check { client := Fin 2 } {}

end Service

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
