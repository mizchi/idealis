// Service (SPEC 5). Clients request and are served; liveness under weak
// fairness of `serve`.
//
// Workaround (sort size): Client is a `const` set fixed to 2 elements by the
// instance module `service2` at the end of the file.
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

  // invariant: holds
  val Disjoint = waiting.intersect(served) == Set()

  // EventuallyServed: written in Quint's temporal fragment, with weak
  // fairness of `serve` as an assumption. `quint run` (the random simulator)
  // does not check temporal properties; only `quint verify --temporal`
  // (Apalache) would, and it was not run (see the report).
  temporal ServeFair = weakFair(anyServe, (waiting, served))
  temporal EventuallyServedPlain =
    CLIENTS.forall(c => always(waiting.contains(c) implies eventually(served.contains(c))))
  // Holds: with fairness.
  temporal EventuallyServed = ServeFair implies EventuallyServedPlain
  // Does not hold without fairness (a behavior that stutters forever after
  // a request); this is EventuallyServedPlain on its own.
}

module service2 {
  import service(CLIENTS = Set("c1", "c2")).*
}
