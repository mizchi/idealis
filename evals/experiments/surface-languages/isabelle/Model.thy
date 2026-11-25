theory Model
  imports Main
begin

(* A transition system: an initial-state predicate and a step relation.
   Actions are relations 's => 's => bool; Next is their disjunction. *)
record 's ts =
  Init :: "'s \<Rightarrow> bool"
  Next :: "'s \<Rightarrow> 's \<Rightarrow> bool"

inductive_set reach :: "'s ts \<Rightarrow> 's set" for T :: "'s ts" where
  init: "Init T s \<Longrightarrow> s \<in> reach T"
| step: "s \<in> reach T \<Longrightarrow> Next T s t \<Longrightarrow> t \<in> reach T"

definition invariant :: "'s ts \<Rightarrow> ('s \<Rightarrow> bool) \<Rightarrow> bool" where
  "invariant T P \<longleftrightarrow> (\<forall>s \<in> reach T. P s)"

definition reachable :: "'s ts \<Rightarrow> ('s \<Rightarrow> bool) \<Rightarrow> bool" where
  "reachable T P \<longleftrightarrow> (\<exists>s \<in> reach T. P s)"

(* A property of steps from reachable states (e.g. "stays shipped"). *)
definition step_invariant :: "'s ts \<Rightarrow> ('s \<Rightarrow> 's \<Rightarrow> bool) \<Rightarrow> bool" where
  "step_invariant T R \<longleftrightarrow> (\<forall>s \<in> reach T. \<forall>t. Next T s t \<longrightarrow> R s t)"

lemma invariantI:
  assumes "\<And>s. Init T s \<Longrightarrow> P s"
      and "\<And>s t. s \<in> reach T \<Longrightarrow> P s \<Longrightarrow> Next T s t \<Longrightarrow> P t"
  shows "invariant T P"
  unfolding invariant_def
proof
  fix s assume "s \<in> reach T" then show "P s" by induct (use assms in auto)
qed

lemma invariant_mono:
  "invariant T P \<Longrightarrow> (\<And>s. P s \<Longrightarrow> Q s) \<Longrightarrow> invariant T Q"
  unfolding invariant_def by blast

lemma unreachable_by_invariant:
  "invariant T P \<Longrightarrow> (\<And>s. P s \<Longrightarrow> \<not> Q s) \<Longrightarrow> \<not> reachable T Q"
  unfolding invariant_def reachable_def by blast

(* Behaviors: infinite traces nat => 's starting in Init, each step is a
   Next step or a stutter (TLA-style). *)
definition behavior :: "'s ts \<Rightarrow> (nat \<Rightarrow> 's) \<Rightarrow> bool" where
  "behavior T \<sigma> \<longleftrightarrow> Init T (\<sigma> 0) \<and> (\<forall>i. Next T (\<sigma> i) (\<sigma> (Suc i)) \<or> \<sigma> (Suc i) = \<sigma> i)"

definition enabled :: "('s \<Rightarrow> 's \<Rightarrow> bool) \<Rightarrow> 's \<Rightarrow> bool" where
  "enabled A s \<longleftrightarrow> (\<exists>t. A s t)"

(* Weak fairness of action A: if A is enabled continuously from i on,
   it is taken at some j >= i. *)
definition WF :: "('s \<Rightarrow> 's \<Rightarrow> bool) \<Rightarrow> (nat \<Rightarrow> 's) \<Rightarrow> bool" where
  "WF A \<sigma> \<longleftrightarrow> (\<forall>i. (\<forall>j\<ge>i. enabled A (\<sigma> j)) \<longrightarrow> (\<exists>j\<ge>i. A (\<sigma> j) (\<sigma> (Suc j))))"

(* [](P --> <>Q) on a trace. *)
definition leads_to :: "('s \<Rightarrow> bool) \<Rightarrow> ('s \<Rightarrow> bool) \<Rightarrow> (nat \<Rightarrow> 's) \<Rightarrow> bool" where
  "leads_to P Q \<sigma> \<longleftrightarrow> (\<forall>i. P (\<sigma> i) \<longrightarrow> (\<exists>j\<ge>i. Q (\<sigma> j)))"

end
