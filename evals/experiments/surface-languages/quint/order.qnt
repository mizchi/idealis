// Order (SPEC 3). An order's status is a sum type with payloads.
//
// Workaround (sort size): Order is a `const` set fixed to 2 elements by the
// instance module `order2` at the end of the file.
module order {
  type OrderId = str
  const ORDERS: Set[OrderId]

  type Carrier = Post | Courier
  type Reason = Customer | Fraud
  type Status = Draft | Placed | Shipped(Carrier) | Cancelled(Reason)

  var status: OrderId -> Status
  // Workaround (ShippedStays): `quint run` checks state invariants only, so
  // this history variable holds the previous state's `status` (equal to
  // `status` in the initial state) and ShippedStays compares the two.
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

  // invariant over (prev, status): holds. A shipped order stays shipped with
  // the same tracking.
  val ShippedStays = ORDERS.forall(o =>
    match prev.get(o) {
      | Shipped(c) => status.get(o) == Shipped(c)
      | _ => true
    })

  // reachable: check `NotFraudCancelled` and expect a violation
  val FraudCancelled = ORDERS.exists(o =>
    match status.get(o) {
      | Cancelled(r) => r == Fraud
      | _ => false
    })
  val NotFraudCancelled = not(FraudCancelled)

  // ShippedCancelled is ruled out by the type: a `Status` value has exactly
  // one constructor, so "Shipped and Cancelled" is not a state that can be
  // written. The closest predicate tests both constructors on the same value
  // and is false by construction (no checking needed):
  //   match s { | Shipped(_) => match s { | Cancelled(_) => true | _ => false } | _ => false }
}

module order2 {
  import order(ORDERS = Set("o1", "o2")).*
}
