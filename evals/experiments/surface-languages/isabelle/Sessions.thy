theory Sessions
  imports Model
begin

(* Sort User is 'u; admins is a fixed configuration argument.
   locked: lone User is an option. *)
record 'u sstate =
  active :: "'u set"
  locked :: "'u option"

(* Workaround: HOL's card is 0 on infinite sets, so "#active < 2" also
   needs "finite active"; active only grows by insert, so it is finite in
   every reachable state (Capacity below carries it). *)
definition login where
  "login u s t \<longleftrightarrow> u \<notin> active s \<and> card (active s) < 2 \<and> t = s\<lparr>active := insert u (active s)\<rparr>"
definition logout where
  "logout u s t \<longleftrightarrow> u \<in> active s \<and> t = s\<lparr>active := active s - {u}\<rparr>"
definition lock where
  "lock admins u s t \<longleftrightarrow> locked s = None \<and> u \<in> admins \<and> u \<in> active s \<and> t = s\<lparr>locked := Some u\<rparr>"
definition unlock where
  "unlock u s t \<longleftrightarrow> locked s = Some u \<and> t = s\<lparr>locked := None\<rparr>"

definition sessions :: "'u set \<Rightarrow> 'u sstate ts" where
  "sessions admins =
     \<lparr> Init = (\<lambda>s. active s = {} \<and> locked s = None),
       Next = (\<lambda>s t. \<exists>u. login u s t \<or> logout u s t \<or> lock admins u s t \<or> unlock u s t) \<rparr>"

definition Capacity :: "'u sstate \<Rightarrow> bool" where
  "Capacity s \<longleftrightarrow> finite (active s) \<and> card (active s) \<le> 2"
definition LockedByAdmin where
  "LockedByAdmin admins s \<longleftrightarrow> (\<forall>u. locked s = Some u \<longrightarrow> u \<in> admins)"
definition LockedIsActive :: "'u sstate \<Rightarrow> bool" where
  "LockedIsActive s \<longleftrightarrow> (\<forall>u. locked s = Some u \<longrightarrow> u \<in> active s)"
definition Full :: "'u sstate \<Rightarrow> bool" where
  "Full s \<longleftrightarrow> card (active s) = 2 \<and> locked s \<noteq> None"
definition ThreeActive :: "'u sstate \<Rightarrow> bool" where
  "ThreeActive s \<longleftrightarrow> card (active s) = 3"

(* The fact "some admins" is the hypothesis admins \<noteq> {}. *)
lemma Capacity: "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) Capacity"
  by (rule invariantI)
     (auto simp: sessions_def Capacity_def login_def logout_def lock_def unlock_def)

lemma LockedByAdmin: "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) (LockedByAdmin admins)"
  by (rule invariantI)
     (auto simp: sessions_def LockedByAdmin_def login_def logout_def lock_def unlock_def)

(* LockedIsActive does not hold: login a; lock a; logout a. *)
lemma LockedIsActive_fails:
  fixes a :: 'u
  assumes "a \<in> admins"
  shows "\<not> invariant (sessions admins) LockedIsActive"
proof -
  let ?T = "sessions admins"
  let ?s0 = "\<lparr>active = {}, locked = None\<rparr> :: 'u sstate"
  let ?s1 = "\<lparr>active = {a}, locked = None\<rparr> :: 'u sstate"
  let ?s2 = "\<lparr>active = {a}, locked = Some a\<rparr> :: 'u sstate"
  let ?s3 = "\<lparr>active = {}, locked = Some a\<rparr> :: 'u sstate"
  have "?s0 \<in> reach ?T" by (rule reach.init) (simp add: sessions_def)
  moreover have "Next ?T ?s0 ?s1" by (auto simp: sessions_def login_def)
  moreover have "Next ?T ?s1 ?s2" using assms by (auto simp: sessions_def lock_def)
  moreover have "Next ?T ?s2 ?s3" by (auto simp: sessions_def logout_def)
  ultimately have "?s3 \<in> reach ?T" by (meson reach.step)
  then show ?thesis
    unfolding invariant_def LockedIsActive_def by (simp add: bexI[of _ ?s3])
qed

(* Full needs two distinct users (a size argument >= 2). *)
lemma Full:
  fixes a b :: 'u
  assumes "a \<in> admins" and "a \<noteq> b"
  shows "reachable (sessions admins) Full"
proof -
  let ?T = "sessions admins"
  let ?s0 = "\<lparr>active = {}, locked = None\<rparr> :: 'u sstate"
  let ?s1 = "\<lparr>active = {a}, locked = None\<rparr> :: 'u sstate"
  let ?s2 = "\<lparr>active = {b, a}, locked = None\<rparr> :: 'u sstate"
  let ?s3 = "\<lparr>active = {b, a}, locked = Some a\<rparr> :: 'u sstate"
  have "?s0 \<in> reach ?T" by (rule reach.init) (simp add: sessions_def)
  moreover have "Next ?T ?s0 ?s1" by (auto simp: sessions_def login_def)
  moreover have "Next ?T ?s1 ?s2" using assms by (auto simp: sessions_def login_def)
  moreover have "Next ?T ?s2 ?s3" using assms by (auto simp: sessions_def lock_def)
  ultimately have "?s3 \<in> reach ?T" by (meson reach.step)
  then show ?thesis
    unfolding reachable_def Full_def using assms by (intro bexI[of _ ?s3]) auto
qed

lemma ThreeActive: "admins \<noteq> {} \<Longrightarrow> \<not> reachable (sessions admins) ThreeActive"
  by (rule unreachable_by_invariant[OF Capacity]) (auto simp: Capacity_def ThreeActive_def)

end
