theory Permissions
  imports Model
begin

(* Sorts User, Role, Doc are type variables 'u 'r 'd: the model does not
   fix their sizes; lemmas hold for every size (nitpick picks sizes). *)

(* Configuration (does not change) and its fact. *)
definition perm_fact ::
  "('u \<times> 'r) set \<Rightarrow> ('r \<times> 'd) set \<Rightarrow> 'd set \<Rightarrow> 'u set \<Rightarrow> bool" where
  "perm_fact grants reads secret cleared \<longleftrightarrow>
     (\<forall>r d u. (r, d) \<in> reads \<and> d \<in> secret \<and> (u, r) \<in> grants \<longrightarrow> u \<in> cleared)"

(* State: open :: User -> set Doc, as a set of pairs. *)
type_synonym ('u, 'd) pstate = "('u \<times> 'd) set"

(* u's roles can read d: relational join grants ; reads. *)
abbreviation can_read where
  "can_read grants reads u d \<equiv> (u, d) \<in> grants O reads"

definition read_act where
  "read_act grants reads u d s t \<longleftrightarrow> can_read grants reads u d \<and> t = insert (u, d) s"

definition close_act where
  "close_act u d s t \<longleftrightarrow> (u, d) \<in> s \<and> t = s - {(u, d)}"

definition perm_ts ::
  "('u \<times> 'r) set \<Rightarrow> ('r \<times> 'd) set \<Rightarrow> ('u, 'd) pstate ts" where
  "perm_ts grants reads =
     \<lparr> Init = (\<lambda>s. s = {}),
       Next = (\<lambda>s t. \<exists>u d. read_act grants reads u d s t \<or> close_act u d s t) \<rparr>"

definition NoLeak where
  "NoLeak secret cleared s \<longleftrightarrow> (\<forall>u d. (u, d) \<in> s \<and> d \<in> secret \<longrightarrow> u \<in> cleared)"
definition OnlyGranted where
  "OnlyGranted grants reads s \<longleftrightarrow> (\<forall>u d. (u, d) \<in> s \<longrightarrow> can_read grants reads u d)"
definition SecretRead where
  "SecretRead secret s \<longleftrightarrow> (\<exists>u d. (u, d) \<in> s \<and> d \<in> secret)"
definition UnclearedSecret where
  "UnclearedSecret secret cleared s \<longleftrightarrow> (\<exists>u d. (u, d) \<in> s \<and> d \<in> secret \<and> u \<notin> cleared)"

lemma OnlyGranted: "invariant (perm_ts grants reads) (OnlyGranted grants reads)"
  by (rule invariantI) (auto simp: perm_ts_def OnlyGranted_def read_act_def close_act_def)

lemma NoLeak:
  assumes "perm_fact grants reads secret cleared"
  shows "invariant (perm_ts grants reads) (NoLeak secret cleared)"
  using OnlyGranted
  by (rule invariant_mono) (use assms in \<open>unfold OnlyGranted_def NoLeak_def perm_fact_def, blast\<close>)

(* reachable: some configuration satisfying the fact reaches SecretRead
   (rels are chosen by the checker, so this is existential over them). *)
lemma SecretRead:
  "\<exists>(grants :: ('u \<times> 'r) set) (reads :: ('r \<times> 'd) set) secret cleared. perm_fact grants reads secret cleared \<and>
     reachable (perm_ts grants reads) (SecretRead secret)"
proof -
  fix u :: 'u and r :: 'r and d :: 'd
  let ?g = "{(u, r)}" and ?rd = "{(r, d)}"
  have "{(u, d)} \<in> reach (perm_ts ?g ?rd)"
    by (rule reach.step[OF reach.init, of _ "{}"]) (auto simp: perm_ts_def read_act_def)
  then show ?thesis
    by (intro exI[of _ ?g] exI[of _ ?rd] exI[of _ "{d}"] exI[of _ "{u}"])
       (auto simp: perm_fact_def reachable_def SecretRead_def)
qed

lemma UnclearedSecret:
  "perm_fact grants reads secret cleared \<Longrightarrow>
   \<not> reachable (perm_ts grants reads) (UnclearedSecret secret cleared)"
  by (rule unreachable_by_invariant[OF NoLeak]) (auto simp: NoLeak_def UnclearedSecret_def)

end
