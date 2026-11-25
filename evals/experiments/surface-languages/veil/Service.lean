module

public import Veil

/-! Service (SPEC 5) in Veil: a safety invariant and a liveness property
under weak fairness. Temporal formulas are Lentil's TLA (`□ ◇ 𝒲ℱ`). -/

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

-- Workaround: TLA behaviours may stutter, but Veil's behaviours take only
-- the declared actions (a dead end alone stutters). Without this action the
-- unfair behaviour "request c, then do nothing forever" does not exist, and
-- EventuallyServed would hold even without fairness. (Same idiom as
-- Examples/LivenessMC/TLAPlus/HourClock.lean.)
action stutter { pure () }

safety [disjoint] ∀ c, ¬ (waiting c ∧ served c)

-- `#temporal_check` checks in declaration order and stops at the first
-- counterexample, so the property expected to hold comes first.

-- EventuallyServed with weak fairness on each `serve c`.
temporal [eventually_served_fair]
  (∀ c : client, 𝒲ℱ (serve c)) →
    ∀ c : client, □ (⌜ waiting c ⌝ → ◇ ⌜ served c ⌝)

-- The same without fairness: expected to fail (request, then stutter).
temporal [eventually_served]
  ∀ c : client, □ (⌜ waiting c ⌝ → ◇ ⌜ served c ⌝)

#gen_spec
#gen_executable

#model_check { client := Fin 2 } {}

#check_invariants

set_option veil.violationIsError false in
#temporal_check { client := Fin 2 } {}

end Service
