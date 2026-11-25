// Sessions (SPEC 4). At most two active users; an admin may lock.
//
// Workaround (sort size): User is a `const` set fixed to 3 elements by the
// instance module `sessions3` at the end of the file.
// Workaround (configuration): `admins` is chosen nondeterministically in
// `init` among the non-empty subsets and kept unchanged by every action.
// `lone User` is written as a sum type.
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

  // invariant: holds
  val Capacity = active.size() <= 2
  // invariant: holds
  val LockedByAdmin = match locked {
    | Unlocked => true
    | LockedBy(u) => admins.contains(u)
  }
  // invariant: does NOT hold (logout while holding the lock)
  val LockedIsActive = match locked {
    | Unlocked => true
    | LockedBy(u) => active.contains(u)
  }
  // reachable: check `NotFull` and expect a violation
  val Full = active.size() == 2 and locked != Unlocked
  val NotFull = not(Full)
  // unreachable: check `NotThreeActive` and expect it to hold
  val ThreeActive = active.size() == 3
  val NotThreeActive = not(ThreeActive)
}

module sessions3 {
  import sessions(USERS = Set("u1", "u2", "u3")).*
}
