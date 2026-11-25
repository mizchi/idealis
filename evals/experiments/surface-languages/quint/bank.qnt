// Bank (SPEC 2). Deposits and withdrawals on accounts with a cap of 4.
//
// Workaround (sort size): Account is a `const` set fixed to 2 elements by
// the instance module `bank2` at the end of the file.
// Integers are Quint's unbounded `int`. The action arguments `n` are drawn
// from -4..4 only in `step` (the simulator needs a finite range to sample
// from); the model itself does not bound `int`.
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

  // invariant: holds
  val NonNegative = ACCOUNTS.forall(a => balance.get(a) >= 0)
  // reachable (in one step): check `NotRich` and expect a violation
  val Rich = ACCOUNTS.exists(a => balance.get(a) >= 3)
  val NotRich = not(Rich)
}

module bank2 {
  import bank(ACCOUNTS = Set("a1", "a2")).*
}
