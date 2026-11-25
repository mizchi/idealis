You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Model permissions

```
model Permissions
sig User
sig Role
sig Doc
rel grants: User -> set Role
rel reads: Role -> set Doc
rel secret: set Doc
rel cleared: set User
fact { all r: Role | some r.reads & secret implies grants.r in cleared }
var open: User -> set Doc
init { no open }
action read(u: User, d: Doc) {
  d in u.grants.reads
  open' = open + u -> d
}
action close(u: User, d: Doc) {
  d in u.open
  open' = open - u -> d
}
invariant NoLeak { all u: User | some u.open & secret implies u in cleared }
invariant OnlyGranted { all u: User | u.open in u.grants.reads }
reachable SecretRead { some u: User | some u.open & secret }
reachable UnclearedSecret { some u: User | u not in cleared and some u.open & secret }
scope User = 2, Role = 2, Doc = 2, steps = 6

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
model Bank
sig Account
var balance: Account -> one Int
init { all a: Account | a.balance = 0 }
action deposit(a: Account, n: Int) {
  n > 0
  a.balance + n <= 4
  balance' = balance - a -> a.balance + a -> (a.balance + n)
}
action withdraw(a: Account, n: Int) {
  n > 0
  a.balance >= n
  balance' = balance - a -> a.balance + a -> (a.balance - n)
}
invariant NonNegative { all a: Account | a.balance >= 0 }
reachable Rich { some a: Account | a.balance >= 3 }
scope Account = 2, Int = -4..4, steps = 4

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
model Order
sig Order
enum Status = Draft | Placed | Shipped | Cancelled
enum Carrier = Post | Courier
enum Reason = Customer | Fraud
var status: Order -> one Status
var tracking: Order -> lone Carrier
var reason: Order -> lone Reason
init { all o: Order | o.status = Draft }
init { no tracking and no reason }
action place(o: Order) {
  o.status = Draft
  status' = status - o -> Draft + o -> Placed
}
action ship(o: Order, c: Carrier) {
  o.status = Placed
  status' = status - o -> Placed + o -> Shipped
  tracking' = tracking + o -> c
}
action cancel(o: Order, r: Reason) {
  o.status in Draft + Placed
  status' = status - o -> o.status + o -> Cancelled
  reason' = reason + o -> r
}
invariant PayloadsMatch {
  all o: Order | {
    o.status = Shipped implies some o.tracking
    some o.tracking implies o.status = Shipped
    o.status = Cancelled implies some o.reason
    some o.reason implies o.status = Cancelled
  }
}
reachable FraudCancelled { some o: Order | o.status = Cancelled and o.reason = Fraud }
reachable ShippedCancelled { some o: Order | o.status = Shipped and o.status = Cancelled }
scope Order = 2, steps = 4

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
model Sessions
sig User
rel admins: set User
fact { some admins }
var active: set User
var locked: lone User
init { no active and no locked }
action login(u: User) {
  u not in active
  #active < 2
  active' = active + u
}
action logout(u: User) {
  u in active
  active' = active - u
}
action lock(u: User) {
  no locked
  u in admins & active
  locked' = u
}
action unlock(u: User) {
  u in locked
  locked' = none
}
invariant Capacity { #active <= 2 }
invariant LockedByAdmin { locked in admins }
invariant LockedIsActive { locked in active }
reachable Full { #active = 2 and some locked }
reachable ThreeActive { #active = 3 }
scope User = 3, steps = 8

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
model Service
sig Client
var waiting: set Client
var served: set Client
init { no waiting and no served }
action request(c: Client) {
  c not in waiting + served
  waiting' = waiting + c
}
action serve(c: Client) {
  c in waiting
  waiting' = waiting - c
  served' = served + c
}
invariant Disjoint { no waiting & served }
reachable AllServed { served = Client }
scope Client = 2, steps = 4

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
