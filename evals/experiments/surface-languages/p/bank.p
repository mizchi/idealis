// P int arithmetic is bounded by the native representation; the model stays small.
enum Account { A0, A1 }

machine Bank {
  var balance: map[Account, int];

  fun deposit(account: Account, amount: int) {
    if (amount > 0 && balance[account] + amount <= 4) {
      balance[account] = balance[account] + amount;
    }
  }

  fun withdraw(account: Account, amount: int) {
    if (amount > 0 && balance[account] >= amount) {
      balance[account] = balance[account] - amount;
    }
  }

  start state Init {
    entry {
      var accounts: set[Account];
      var account: Account;
      var amount: int;
      var i: int;
      accounts += (A0);
      accounts += (A1);
      balance[A0] = 0;
      balance[A1] = 0;
      deposit(A0, 3);
      assert balance[A0] >= 3, "Rich witness";
      balance[A0] = 0;

      while (i < 12) {
        account = choose(accounts);
        amount = choose(9) - 4;
        if (choose()) {
          deposit(account, amount);
        } else {
          withdraw(account, amount);
        }
        assert balance[A0] >= 0 && balance[A1] >= 0, "NonNegative";
        i = i + 1;
      }
    }
  }
}
test bank [main=Bank]: { Bank };
