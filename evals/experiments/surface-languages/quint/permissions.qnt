// Permissions (SPEC 1). Users open documents that one of their roles reads.
//
// Workaround (sort sizes): Quint has no scoped uninterpreted sorts. The
// sorts are `const` sets of strings and the size is fixed by the instance
// module `permissions2` at the end of the file (2 elements each).
//
// Workaround (configuration): the configuration relations are not fixed
// constants. They are chosen nondeterministically in `init` among all
// configurations that satisfy the premise, and every action keeps them
// unchanged, so the checks range over every valid configuration.
module permissions {
  type User = str
  type Role = str
  type Doc = str

  const USERS: Set[User]
  const ROLES: Set[Role]
  const DOCS: Set[Doc]

  // Configuration (never changes after init).
  var grants: User -> Set[Role]
  var reads: Role -> Set[Doc]
  var secret: Set[Doc]
  var cleared: Set[User]
  // State.
  var open: User -> Set[Doc]

  // Premise: a role that reads a secret document is granted only to
  // cleared users.
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

  // invariant: holds
  val NoLeak = USERS.forall(u => open.get(u).intersect(secret) != Set() implies cleared.contains(u))
  // invariant: holds
  val OnlyGranted = USERS.forall(u => open.get(u).subseteq(readable(u)))
  // reachable: check `not SecretRead` and expect a violation
  val SecretRead = USERS.exists(u => open.get(u).intersect(secret) != Set())
  val NotSecretRead = not(SecretRead)
  // unreachable: check `NotUnclearedSecret` and expect it to hold
  val UnclearedSecret = USERS.exists(u => not(cleared.contains(u)) and open.get(u).intersect(secret) != Set())
  val NotUnclearedSecret = not(UnclearedSecret)
}

module permissions2 {
  import permissions(USERS = Set("u1", "u2"), ROLES = Set("r1", "r2"), DOCS = Set("d1", "d2")).*
}
