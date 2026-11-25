You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Shared definitions (Model.thy)

```
theory Model
  imports Main
begin

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

definition behavior :: "'s ts \<Rightarrow> (nat \<Rightarrow> 's) \<Rightarrow> bool" where
  "behavior T \<sigma> \<longleftrightarrow> Init T (\<sigma> 0) \<and> (\<forall>i. Next T (\<sigma> i) (\<sigma> (Suc i)) \<or> \<sigma> (Suc i) = \<sigma> i)"

definition enabled :: "('s \<Rightarrow> 's \<Rightarrow> bool) \<Rightarrow> 's \<Rightarrow> bool" where
  "enabled A s \<longleftrightarrow> (\<exists>t. A s t)"

definition WF :: "('s \<Rightarrow> 's \<Rightarrow> bool) \<Rightarrow> (nat \<Rightarrow> 's) \<Rightarrow> bool" where
  "WF A \<sigma> \<longleftrightarrow> (\<forall>i. (\<forall>j\<ge>i. enabled A (\<sigma> j)) \<longrightarrow> (\<exists>j\<ge>i. A (\<sigma> j) (\<sigma> (Suc j))))"

definition leads_to :: "('s \<Rightarrow> bool) \<Rightarrow> ('s \<Rightarrow> bool) \<Rightarrow> (nat \<Rightarrow> 's) \<Rightarrow> bool" where
  "leads_to P Q \<sigma> \<longleftrightarrow> (\<forall>i. P (\<sigma> i) \<longrightarrow> (\<exists>j\<ge>i. Q (\<sigma> j)))"

end

```

## Model permissions

```
theory Permissions
  imports Model
begin

definition perm_fact ::
  "('u \<times> 'r) set \<Rightarrow> ('r \<times> 'd) set \<Rightarrow> 'd set \<Rightarrow> 'u set \<Rightarrow> bool" where
  "perm_fact grants reads secret cleared \<longleftrightarrow>
     (\<forall>r d u. (r, d) \<in> reads \<and> d \<in> secret \<and> (u, r) \<in> grants \<longrightarrow> u \<in> cleared)"

type_synonym ('u, 'd) pstate = "('u \<times> 'd) set"

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

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
theory Bank
  imports Model
begin

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

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
theory Order
  imports Model
begin

datatype carrier = Post | Courier
datatype reason = Customer | Fraud
datatype status = Draft | Placed | Shipped carrier | Cancelled reason

type_synonym 'k ostate = "'k \<Rightarrow> status"

definition place where
  "place k s t \<longleftrightarrow> s k = Draft \<and> t = s(k := Placed)"
definition ship where
  "ship k c s t \<longleftrightarrow> s k = Placed \<and> t = s(k := Shipped c)"
fun cancellable :: "status \<Rightarrow> bool" where
  "cancellable Draft = True"
| "cancellable Placed = True"
| "cancellable _ = False"

definition cancel where
  "cancel k r s t \<longleftrightarrow> cancellable (s k) \<and> t = s(k := Cancelled r)"

definition order :: "'k ostate ts" where
  "order = \<lparr> Init = (\<lambda>s. \<forall>k. s k = Draft),
             Next = (\<lambda>s t. \<exists>k. place k s t \<or> (\<exists>c. ship k c s t) \<or> (\<exists>r. cancel k r s t)) \<rparr>"

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

definition ShippedCancelled :: "'k ostate \<Rightarrow> bool" where
  "ShippedCancelled s \<longleftrightarrow> (\<exists>k c r. s k = Shipped c \<and> s k = Cancelled r)"

lemma ShippedCancelled: "\<not> reachable (order :: 'k ostate ts) ShippedCancelled"
  by (auto simp: reachable_def ShippedCancelled_def)

end

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
theory Sessions
  imports Model
begin

record 'u sstate =
  active :: "'u set"
  locked :: "'u option"

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

lemma Capacity: "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) Capacity"
  by (rule invariantI)
     (auto simp: sessions_def Capacity_def login_def logout_def lock_def unlock_def)

lemma LockedByAdmin: "admins \<noteq> {} \<Longrightarrow> invariant (sessions admins) (LockedByAdmin admins)"
  by (rule invariantI)
     (auto simp: sessions_def LockedByAdmin_def login_def logout_def lock_def unlock_def)

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

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
theory Service
  imports Model
begin

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

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
