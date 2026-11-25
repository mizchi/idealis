theory Order
  imports Model
begin

datatype carrier = Post | Courier
datatype reason = Customer | Fraud
datatype status = Draft | Placed | Shipped carrier | Cancelled reason

(* Sort Order is 'k. *)
type_synonym 'k ostate = "'k \<Rightarrow> status"

definition place where
  "place k s t \<longleftrightarrow> s k = Draft \<and> t = s(k := Placed)"
definition ship where
  "ship k c s t \<longleftrightarrow> s k = Placed \<and> t = s(k := Shipped c)"
(* Guard by pattern matching (equivalent to case; gives one simp rule per
   constructor, which the proofs below need). *)
fun cancellable :: "status \<Rightarrow> bool" where
  "cancellable Draft = True"
| "cancellable Placed = True"
| "cancellable _ = False"

definition cancel where
  "cancel k r s t \<longleftrightarrow> cancellable (s k) \<and> t = s(k := Cancelled r)"

definition order :: "'k ostate ts" where
  "order = \<lparr> Init = (\<lambda>s. \<forall>k. s k = Draft),
             Next = (\<lambda>s t. \<exists>k. place k s t \<or> (\<exists>c. ship k c s t) \<or> (\<exists>r. cancel k r s t)) \<rparr>"

(* A property of steps; the payload is taken with case, not a selector. *)
definition ShippedStays :: "'k ostate \<Rightarrow> 'k ostate \<Rightarrow> bool" where
  "ShippedStays s t \<longleftrightarrow>
     (\<forall>k. case s k of Shipped c \<Rightarrow> t k = Shipped c | _ \<Rightarrow> True)"

definition FraudCancelled :: "'k ostate \<Rightarrow> bool" where
  "FraudCancelled s \<longleftrightarrow> (\<exists>k. case s k of Cancelled Fraud \<Rightarrow> True | _ \<Rightarrow> False)"

lemma ShippedStays: "step_invariant order ShippedStays"
  unfolding step_invariant_def
  by (auto simp: order_def ShippedStays_def place_def ship_def cancel_def
           split: status.splits)

lemma FraudCancelled: "reachable (order :: 'k ostate ts) FraudCancelled"
proof -
  fix k :: 'k
  have "(\<lambda>_. Draft)(k := Cancelled Fraud) \<in> reach order"
    by (rule reach.step[OF reach.init, of _ "\<lambda>_. Draft"]) (auto simp: order_def cancel_def)
  then show ?thesis
    unfolding reachable_def FraudCancelled_def by (intro bexI[of _ "(\<lambda>_. Draft)(k := Cancelled Fraud)"]) auto
qed

(* ShippedCancelled: typeable in HOL (it is not a type error), but it is
   false for every state by constructor distinctness, so it is unreachable
   without looking at the transition system. *)
definition ShippedCancelled :: "'k ostate \<Rightarrow> bool" where
  "ShippedCancelled s \<longleftrightarrow> (\<exists>k c r. s k = Shipped c \<and> s k = Cancelled r)"

lemma ShippedCancelled: "\<not> reachable (order :: 'k ostate ts) ShippedCancelled"
  by (auto simp: reachable_def ShippedCancelled_def)

end
