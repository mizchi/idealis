# 書き比べの題材（plan 0107）

各書き方の書き手が共有する仕様。意味はidealisの核の意味（[docs/kernel-semantics.md](../../../docs/kernel-semantics.md)）に従う: 状態は有限の値、actionはguardが成り立つときに同時に代入し、代入しない状態は変わらない。invariantは到達できるすべての状態で、reachableはある到達できる状態で成り立つか。sigの大きさは検査の引数で、モデルには書かない（書き方が許すなら）。

各題材について、構造（型・状態・初期状態・action）と、下の性質をすべて書く。書けないものは、書けない理由をコメントで残す。回避策で書いたものは、回避策をコメントで説明する。

## 1. permissions

元: `fixtures/rmodel/permissions.model`。

- sort: User・Role・Doc（大きさは2ずつで検査する）。
- 設定（変わらない）: `grants: User -> set Role`、`reads: Role -> set Doc`、`secret: set Doc`、`cleared: set User`。前提: 秘密の文書を読むroleは、clearedな利用者にだけ付与される（すべてのrole r、文書 d、利用者 uで、`reads(r, d) ∧ secret(d) ∧ grants(u, r) → cleared(u)`）。
- 状態: `open: User -> set Doc`、初期は空。
- `read(u, d)`: uのいずれかのroleがdを読めるとき、`open`に`(u, d)`を加える。
- `close(u, d)`: `(u, d)`が`open`にあるとき、除く。
- 性質: `NoLeak`（秘密の文書を開いている利用者はcleared）と`OnlyGranted`（開いている文書は、自分のroleで読めるもの）が成り立つ。`SecretRead`（誰かが秘密の文書を開いている）に到達できる。`UnclearedSecret`（clearedでない利用者が秘密の文書を開いている）に到達できない。

## 2. bank

元: `fixtures/rcheck/bank.model`。

- sort: Account（大きさ2）。整数は数学の整数（検査では-4〜4の範囲を数え上げる）。
- 状態: `balance: Account -> Int`、初期はすべて0。
- `deposit(a, n)`: `n > 0`かつ`balance(a) + n <= 4`のとき、`balance(a)`を`n`増やす。
- `withdraw(a, n)`: `n > 0`かつ`balance(a) >= n`のとき、`n`減らす。
- 性質: `NonNegative`（すべての残高が0以上）が成り立つ。`Rich`（ある口座の残高が3以上）に到達できる（1歩で）。

## 3. order（代数的なデータ型）

新しい題材。

- sort: Order（大きさ2）。
- 値: `Carrier = Post | Courier`、`Reason = Customer | Fraud`、`Status = Draft | Placed | Shipped(tracking: Carrier) | Cancelled(reason: Reason)`。
- 状態: `status: Order -> Status`、初期はすべて`Draft`。
- `place(o)`: `Draft`なら`Placed`に。
- `ship(o, c)`: `Placed`なら`Shipped(c)`に。
- `cancel(o, r)`: `Draft`か`Placed`なら`Cancelled(r)`に（`Shipped`は取り消せない）。
- 性質: `ShippedStays`（`Shipped`の注文は、次の状態でも`Shipped`、同じtrackingのまま）が成り立つ。`FraudCancelled`（`Cancelled(Fraud)`の注文がある）に到達できる。`ShippedCancelled`（ある注文が`Shipped`かつ`Cancelled`）は型として書けないなら、書けないことを記録する。
- 中身（trackingやreason）の取り出しは、`match`かそれに相当する、部分的でない書き方で。

## 4. sessions

元: `fixtures/rmodel/sessions.model`。

- sort: User（大きさ3）。設定: `admins: set User`、前提は空でないこと。
- 状態: `active: set User`、`locked: lone User`（0か1人）、初期は両方空。
- `login(u)`: uがactiveでなく、activeの人数が2未満なら加える。
- `logout(u)`: activeなら除く。
- `lock(u)`: lockedが空で、uがadminかつactiveなら、lockedをuに。
- `unlock(u)`: lockedがuなら空に。
- 性質: `Capacity`（activeは2人以下）、`LockedByAdmin`（lockedはadmin）、`LockedIsActive`（lockedはactive）。`LockedIsActive`は**成り立たない**（lockしたまま`logout`できる）。`Full`（2人activeでlockedがいる）に到達できる。`ThreeActive`（3人active）に到達できない。

## 5. service（活性とfairness）

新しい題材。

- sort: Client（大きさ2）。
- 状態: `waiting: set Client`、`served: set Client`、初期は両方空。
- `request(c)`: cがwaitingでもservedでもなければ、waitingに加える。
- `serve(c)`: cがwaitingなら、waitingから除いてservedに加える。
- 性質: `EventuallyServed`（`serve`に弱いfairnessがあるとき、すべてのcについて、cがwaitingならいつかservedになる、`□(c ∈ waiting → ◇ c ∈ served)`）が成り立つ。fairnessがなければ成り立たない（ずっと何もしない、またはずっと`request`だけ、のbehavior）。safetyとして`Disjoint`（waitingとservedは交わらない）。
