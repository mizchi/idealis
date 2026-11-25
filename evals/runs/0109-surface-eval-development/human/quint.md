You are reading models written in a modelling language you may not have seen. Answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Model permissions

```
module permissions {
  type User = str
  type Role = str
  type Doc = str

  const USERS: Set[User]
  const ROLES: Set[Role]
  const DOCS: Set[Doc]

  var grants: User -> Set[Role]
  var reads: Role -> Set[Doc]
  var secret: Set[Doc]
  var cleared: Set[User]
  var open: User -> Set[Doc]

  pure def Premise(g: User -> Set[Role], rd: Role -> Set[Doc], s: Set[Doc], c: Set[User]): bool =
    ROLES.forall(r => DOCS.forall(d => USERS.forall(u =>
      (rd.get(r).contains(d) and s.contains(d) and g.get(u).contains(r)) implies c.contains(u))))

  def readable(u: User): Set[Doc] = grants.get(u).map(r => reads.get(r)).flatten()

  action init = {
    nondet g = USERS.setOfMaps(ROLES.powerset()).oneOf()
    nondet rd = ROLES.setOfMaps(DOCS.powerset()).oneOf()
    nondet s = DOCS.powerset().oneOf()
    nondet c = USERS.powerset().oneOf()
    all {
      Premise(g, rd, s, c),
      grants' = g,
      reads' = rd,
      secret' = s,
      cleared' = c,
      open' = USERS.mapBy(_ => Set()),
    }
  }

  action keepConfig = all {
    grants' = grants,
    reads' = reads,
    secret' = secret,
    cleared' = cleared,
  }

  action read(u: User, d: Doc): bool = all {
    readable(u).contains(d),
    open' = open.set(u, open.get(u).union(Set(d))),
    keepConfig,
  }

  action close(u: User, d: Doc): bool = all {
    open.get(u).contains(d),
    open' = open.set(u, open.get(u).exclude(Set(d))),
    keepConfig,
  }

  action step = {
    nondet u = USERS.oneOf()
    nondet d = DOCS.oneOf()
    any { read(u, d), close(u, d) }
  }

  val NoLeak = USERS.forall(u => open.get(u).intersect(secret) != Set() implies cleared.contains(u))
  val OnlyGranted = USERS.forall(u => open.get(u).subseteq(readable(u)))
  val SecretRead = USERS.exists(u => open.get(u).intersect(secret) != Set())
  val NotSecretRead = not(SecretRead)
  val UnclearedSecret = USERS.exists(u => not(cleared.contains(u)) and open.get(u).intersect(secret) != Set())
  val NotUnclearedSecret = not(UnclearedSecret)
}

module permissions2 {
  import permissions(USERS = Set("u1", "u2"), ROLES = Set("r1", "r2"), DOCS = Set("d1", "d2")).*
}

```

- `permissions.noRole`: Can a user end up with a document open that none of the user's roles can read? Options: "yes", "no"
- `permissions.close`: After a user closes a document the user had open, is that document still open for that user? Options: "yes", "no"
- `permissions.uncleared`: Can a user who is not cleared ever have a secret document open? Options: "yes", "no"
- `permissions.grants`: Can any action change which roles a user is granted? Options: "yes", "no"

## Model bank

```
module bank {
  type Account = str
  const ACCOUNTS: Set[Account]

  var balance: Account -> int

  action init = balance' = ACCOUNTS.mapBy(_ => 0)

  action deposit(a: Account, n: int): bool = all {
    n > 0,
    balance.get(a) + n <= 4,
    balance' = balance.setBy(a, b => b + n),
  }

  action withdraw(a: Account, n: int): bool = all {
    n > 0,
    balance.get(a) >= n,
    balance' = balance.setBy(a, b => b - n),
  }

  action step = {
    nondet a = ACCOUNTS.oneOf()
    nondet n = (-4).to(4).oneOf()
    any { deposit(a, n), withdraw(a, n) }
  }

  val NonNegative = ACCOUNTS.forall(a => balance.get(a) >= 0)
  val Rich = ACCOUNTS.exists(a => balance.get(a) >= 3)
  val NotRich = not(Rich)
}

module bank2 {
  import bank(ACCOUNTS = Set("a1", "a2")).*
}

```

- `bank.cap`: Can a deposit take an account's balance above 4? Options: "yes", "no"
- `bank.overdraw`: With a balance of 2, can a withdrawal of 3 be applied? Options: "yes", "no"
- `bank.steps`: Starting from the initial state, what is the fewest number of actions after which some account has a balance of at least 3? Options: "0", "1", "2", "3", "never"

## Model order

```
module order {
  type OrderId = str
  const ORDERS: Set[OrderId]

  type Carrier = Post | Courier
  type Reason = Customer | Fraud
  type Status = Draft | Placed | Shipped(Carrier) | Cancelled(Reason)

  var status: OrderId -> Status
  var prev: OrderId -> Status

  action init = all {
    status' = ORDERS.mapBy(_ => Draft),
    prev' = ORDERS.mapBy(_ => Draft),
  }

  action place(o: OrderId): bool = all {
    match status.get(o) {
      | Draft => true
      | _ => false
    },
    status' = status.set(o, Placed),
    prev' = status,
  }

  action ship(o: OrderId, c: Carrier): bool = all {
    match status.get(o) {
      | Placed => true
      | _ => false
    },
    status' = status.set(o, Shipped(c)),
    prev' = status,
  }

  action cancel(o: OrderId, r: Reason): bool = all {
    match status.get(o) {
      | Draft => true
      | Placed => true
      | _ => false
    },
    status' = status.set(o, Cancelled(r)),
    prev' = status,
  }

  action step = {
    nondet o = ORDERS.oneOf()
    nondet c = Set(Post, Courier).oneOf()
    nondet r = Set(Customer, Fraud).oneOf()
    any { place(o), ship(o, c), cancel(o, r) }
  }

  val ShippedStays = ORDERS.forall(o =>
    match prev.get(o) {
      | Shipped(c) => status.get(o) == Shipped(c)
      | _ => true
    })

  val FraudCancelled = ORDERS.exists(o =>
    match status.get(o) {
      | Cancelled(r) => r == Fraud
      | _ => false
    })
  val NotFraudCancelled = not(FraudCancelled)

}

module order2 {
  import order(ORDERS = Set("o1", "o2")).*
}

```

- `order.shippedCancel`: Can an order that has been shipped later become cancelled? Options: "yes", "no"
- `order.draftShip`: Can an order go directly from draft to shipped in one action? Options: "yes", "no"
- `order.tracking`: After an order is shipped with the carrier Post, which carrier does its state record? Options: "Post", "Courier", "none"
- `order.fraud`: Can an order that was placed be cancelled with the reason Fraud? Options: "yes", "no"

## Model sessions

```
module sessions {
  type User = str
  const USERS: Set[User]

  type Lock = Unlocked | LockedBy(User)

  var admins: Set[User]
  var active: Set[User]
  var locked: Lock

  action init = {
    nondet adm = USERS.powerset().exclude(Set(Set())).oneOf()
    all {
      admins' = adm,
      active' = Set(),
      locked' = Unlocked,
    }
  }

  action login(u: User): bool = all {
    not(active.contains(u)),
    active.size() < 2,
    active' = active.union(Set(u)),
    admins' = admins,
    locked' = locked,
  }

  action logout(u: User): bool = all {
    active.contains(u),
    active' = active.exclude(Set(u)),
    admins' = admins,
    locked' = locked,
  }

  action lock(u: User): bool = all {
    locked == Unlocked,
    admins.contains(u),
    active.contains(u),
    locked' = LockedBy(u),
    admins' = admins,
    active' = active,
  }

  action unlock(u: User): bool = all {
    locked == LockedBy(u),
    locked' = Unlocked,
    admins' = admins,
    active' = active,
  }

  action step = {
    nondet u = USERS.oneOf()
    any { login(u), logout(u), lock(u), unlock(u) }
  }

  val Capacity = active.size() <= 2
  val LockedByAdmin = match locked {
    | Unlocked => true
    | LockedBy(u) => admins.contains(u)
  }
  val LockedIsActive = match locked {
    | Unlocked => true
    | LockedBy(u) => active.contains(u)
  }
  val Full = active.size() == 2 and locked != Unlocked
  val NotFull = not(Full)
  val ThreeActive = active.size() == 3
  val NotThreeActive = not(ThreeActive)
}

module sessions3 {
  import sessions(USERS = Set("u1", "u2", "u3")).*
}

```

- `sessions.max`: What is the largest number of users that can be active at the same time? Options: "1", "2", "3"
- `sessions.lockedInactive`: Can there be a locked user who is not active? Options: "yes", "no"
- `sessions.nonAdmin`: Can a user who is not an admin become the locked user? Options: "yes", "no"
- `sessions.twoLocked`: Can two different users be locked at the same time? Options: "yes", "no"

## Model service

```
module service {
  type Client = str
  const CLIENTS: Set[Client]

  var waiting: Set[Client]
  var served: Set[Client]

  action init = all {
    waiting' = Set(),
    served' = Set(),
  }

  action request(c: Client): bool = all {
    not(waiting.contains(c)),
    not(served.contains(c)),
    waiting' = waiting.union(Set(c)),
    served' = served,
  }

  action serve(c: Client): bool = all {
    waiting.contains(c),
    waiting' = waiting.exclude(Set(c)),
    served' = served.union(Set(c)),
  }

  action anyServe = { nondet c = CLIENTS.oneOf() serve(c) }

  action step = {
    nondet c = CLIENTS.oneOf()
    any { request(c), serve(c) }
  }

  val Disjoint = waiting.intersect(served) == Set()

  temporal ServeFair = weakFair(anyServe, (waiting, served))
  temporal EventuallyServedPlain =
    CLIENTS.forall(c => always(waiting.contains(c) implies eventually(served.contains(c))))
  temporal EventuallyServed = ServeFair implies EventuallyServedPlain
}

module service2 {
  import service(CLIENTS = Set("c1", "c2")).*
}

```

- `service.both`: Can a client be waiting and served at the same time? Options: "yes", "no"
- `service.unfair`: If nothing forces the serve action to happen, must every waiting client eventually be served? Options: "yes", "no"
- `service.again`: Can a client that has been served make a new request? Options: "yes", "no"
