theory Service
  imports Model
begin

(* Sort Client is 'c. *)
record 'c vstate =
  waiting :: "'c set"
  served :: "'c set"

definition request where
  "request c s t \<longleftrightarrow> c \<notin> waiting s \<and> c \<notin> served s \<and> t = s\<lparr>waiting := insert c (waiting s)\<rparr>"
definition serve where
  "serve c s t \<longleftrightarrow> c \<in> waiting s \<and> t = \<lparr>waiting = waiting s - {c}, served = insert c (served s)\<rparr>"

definition service :: "'c vstate ts" where
  "service = \<lparr> Init = (\<lambda>s. waiting s = {} \<and> served s = {}),
               Next = (\<lambda>s t. \<exists>c. request c s t \<or> serve c s t) \<rparr>"

definition Disjoint :: "'c vstate \<Rightarrow> bool" where
  "Disjoint s \<longleftrightarrow> waiting s \<inter> served s = {}"

lemma Disjoint: "invariant service Disjoint"
  by (rule invariantI) (auto simp: service_def Disjoint_def request_def serve_def)

definition EventuallyServed :: "(nat \<Rightarrow> 'c vstate) \<Rightarrow> bool" where
  "EventuallyServed \<sigma> \<longleftrightarrow> (\<forall>c. leads_to (\<lambda>s. c \<in> waiting s) (\<lambda>s. c \<in> served s) \<sigma>)"

(* Fairness: weak fairness of serve c for every client c (per-parameter
   WF, as WF_vars(serve(c)) for each c in TLA+). *)
lemma EventuallyServed_fair:
  assumes beh: "behavior service \<sigma>" and wf: "\<forall>c. WF (serve c) \<sigma>"
  shows "EventuallyServed \<sigma>"
  unfolding EventuallyServed_def leads_to_def
proof (intro allI impI)
  fix c i assume w: "c \<in> waiting (\<sigma> i)"
  show "\<exists>j\<ge>i. c \<in> served (\<sigma> j)"
  proof (rule ccontr)
    assume ns: "\<not> (\<exists>j\<ge>i. c \<in> served (\<sigma> j))"
    have stay: "c \<in> waiting (\<sigma> (i + n))" for n
    proof (induct n)
      case 0 show ?case using w by simp
    next
      case (Suc n)
      have "c \<notin> served (\<sigma> (Suc (i + n)))" using ns by auto
      moreover have "Next service (\<sigma> (i + n)) (\<sigma> (Suc (i + n))) \<or> \<sigma> (Suc (i + n)) = \<sigma> (i + n)"
        using beh by (simp add: behavior_def)
      ultimately show ?case using Suc
        by (auto simp: service_def request_def serve_def)
    qed
    have "\<forall>j\<ge>i. enabled (serve c) (\<sigma> j)"
    proof (intro allI impI)
      fix j assume "i \<le> j"
      then obtain n where "j = i + n" using le_Suc_ex by blast
      then show "enabled (serve c) (\<sigma> j)"
        using stay[of n] by (auto simp: enabled_def serve_def)
    qed
    then obtain j where "j \<ge> i" "serve c (\<sigma> j) (\<sigma> (Suc j))"
      using wf unfolding WF_def by blast
    then show False using ns by (auto simp: serve_def intro: le_SucI)
  qed
qed

(* Without fairness it fails: request c, then stutter forever. *)
lemma EventuallyServed_unfair:
  "\<not> (\<forall>\<sigma>. behavior (service :: 'c vstate ts) \<sigma> \<longrightarrow> EventuallyServed \<sigma>)"
proof -
  fix c :: 'c
  let ?s0 = "\<lparr>waiting = {}, served = {}\<rparr> :: 'c vstate"
  let ?s1 = "\<lparr>waiting = {c}, served = {}\<rparr> :: 'c vstate"
  let ?\<sigma> = "\<lambda>i::nat. if i = 0 then ?s0 else ?s1"
  have "behavior service ?\<sigma>"
    by (auto simp: behavior_def service_def request_def)
  moreover have "\<not> EventuallyServed ?\<sigma>"
    by (auto simp: EventuallyServed_def leads_to_def intro!: exI[of _ c] exI[of _ 1])
  ultimately show ?thesis by blast
qed

end
