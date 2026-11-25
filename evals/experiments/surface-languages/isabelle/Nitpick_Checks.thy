theory Nitpick_Checks
  imports Permissions Bank Order Sessions Service
begin

(* Finite-scope searches with Nitpick. The expect option makes the build
   fail unless Nitpick reports that outcome, so each result is checked. *)

nitpick_params [timeout = 60, max_threads = 1]

(* permissions: a counterexample to "not reachable" is a witness
   (config and state; the fact is an assumption). *)
lemma "perm_fact grants reads secret cleared \<Longrightarrow>
       \<not> reachable (perm_ts grants reads) (SecretRead secret :: ('u, 'd) pstate \<Rightarrow> bool)"
  nitpick [expect = genuine, card 'u = 1-2, card 'r = 1-2, card 'd = 1-2]
  oops

(* bank: without unfolding, Nitpick ran out of time (60 s, 10 of 20
   scopes); with the definitions unfolded it finds a witness. *)
lemma "\<not> reachable (bank :: 'a bstate ts) Rich"
  unfolding reachable_def bank_def Rich_def deposit_def withdraw_def
  nitpick [expect = genuine, card 'a = 1-2]
  oops

lemma "\<not> reachable (order :: 'k ostate ts) FraudCancelled"
  nitpick [expect = genuine, card 'k = 1-2]
  oops

(* sessions: on the folded statement Nitpick wrongly reports "none"
   (observed for invariant and for a hand-unrolled 3-step path); after
   unfolding sessions_def it finds the counterexample. *)
lemma "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) (LockedIsActive :: 'u sstate \<Rightarrow> bool)"
  nitpick [expect = none, card 'u = 1-2]
  oops

lemma "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) (LockedIsActive :: 'u sstate \<Rightarrow> bool)"
  unfolding sessions_def
  nitpick [expect = genuine, card 'u = 1-3]
  oops

(* Full: only a "potentially spurious" witness (card is approximated). *)
lemma "admins \<noteq> {} \<Longrightarrow> \<not> reachable (sessions admins) (Full :: 'u sstate \<Rightarrow> bool)"
  unfolding reachable_def sessions_def Full_def login_def logout_def lock_def unlock_def
  nitpick [expect = potential, card 'u = 1-3]
  oops

(* Capacity is proved (Sessions.Capacity), yet Nitpick reports a
   potentially spurious counterexample: a false alarm from card. *)
lemma "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) (Capacity :: 'u sstate \<Rightarrow> bool)"
  unfolding invariant_def sessions_def Capacity_def login_def logout_def lock_def unlock_def
  nitpick [expect = potential, card 'u = 1-3]
  oops

(* service: the unfair counterexample is an infinite trace nat => state;
   Nitpick warns the goal is outside its fragment and finds none. The
   failure is proved instead (Service.EventuallyServed_unfair). *)
lemma "behavior (service :: 'c vstate ts) \<sigma> \<Longrightarrow> EventuallyServed \<sigma>"
  nitpick [expect = none, card 'c = 1-2]
  oops

end
