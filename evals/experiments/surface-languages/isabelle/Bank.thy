theory Bank
  imports Model
begin

(* Sort Account is 'a. Int is HOL's unbounded int: no -4..4 range is
   needed for the proofs (the range is a checker argument). *)
type_synonym 'a bstate = "'a \<Rightarrow> int"

definition deposit :: "'a \<Rightarrow> int \<Rightarrow> 'a bstate \<Rightarrow> 'a bstate \<Rightarrow> bool" where
  "deposit a n b b' \<longleftrightarrow> n > 0 \<and> b a + n \<le> 4 \<and> b' = b(a := b a + n)"
definition withdraw :: "'a \<Rightarrow> int \<Rightarrow> 'a bstate \<Rightarrow> 'a bstate \<Rightarrow> bool" where
  "withdraw a n b b' \<longleftrightarrow> n > 0 \<and> b a \<ge> n \<and> b' = b(a := b a - n)"

definition bank :: "'a bstate ts" where
  "bank = \<lparr> Init = (\<lambda>b. \<forall>a. b a = 0),
            Next = (\<lambda>b b'. \<exists>a n. deposit a n b b' \<or> withdraw a n b b') \<rparr>"

definition NonNegative :: "'a bstate \<Rightarrow> bool" where "NonNegative b \<longleftrightarrow> (\<forall>a. b a \<ge> 0)"
definition Rich :: "'a bstate \<Rightarrow> bool" where "Rich b \<longleftrightarrow> (\<exists>a. b a \<ge> 3)"

lemma NonNegative: "invariant bank NonNegative"
  by (rule invariantI) (auto simp: bank_def NonNegative_def deposit_def withdraw_def)

(* Depth 1: an initial state and one deposit. *)
lemma Rich_depth1: "\<exists>b b'. Init bank b \<and> Next bank b b' \<and> Rich b'"
proof -
  fix a :: 'a
  show ?thesis
    by (rule exI[of _ "\<lambda>_. 0"], rule exI[of _ "(\<lambda>_. 0)(a := 3)"])
       (auto simp: bank_def deposit_def Rich_def intro!: exI[of _ a] exI[of _ 3])
qed

lemma Rich: "reachable (bank :: 'a bstate ts) Rich"
  using Rich_depth1 unfolding reachable_def by (blast intro: reach.intros)

end
