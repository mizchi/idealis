enum SessionUser { U0, U1, U2 }

machine Sessions {
  var admins: set[SessionUser];
  var active: set[SessionUser];
  // A boolean and a scalar encode the optional reference.
  var hasLock: bool;
  var locked: SessionUser;

  fun login(u: SessionUser) {
    if (!(u in active) && sizeof(active) < 2) {
      active += (u);
    }
  }

  fun logout(u: SessionUser) {
    if (u in active) {
      active -= (u);
    }
  }

  fun lock(u: SessionUser) {
    if (!hasLock && u in admins && u in active) {
      hasLock = true;
      locked = u;
    }
  }

  fun unlock(u: SessionUser) {
    if (hasLock && locked == u) {
      hasLock = false;
    }
  }

  start state Init {
    entry {
      var users: set[SessionUser];
      var u: SessionUser;
      var action: int;
      var i: int;
      var empty: set[SessionUser];

      users += (U0);
      users += (U1);
      users += (U2);
      admins += (U0);
      login(U0);
      lock(U0);
      logout(U0);
      assert hasLock && !(locked in active), "LockedIsActive counterexample";
      login(U1);
      login(U2);
      assert sizeof(active) == 2 && hasLock, "Full witness";
      active = empty;
      hasLock = false;
      admins = empty;

      foreach (u in users) {
        if (choose()) {
          admins += (u);
        }
      }
      if (sizeof(admins) == 0) {
        admins += (choose(users));
      }

      while (i < 12) {
        u = choose(users);
        action = choose(4);
        if (action == 0) {
          login(u);
        } else if (action == 1) {
          logout(u);
        } else if (action == 2) {
          lock(u);
        } else {
          unlock(u);
        }
        assert sizeof(active) <= 2, "Capacity / ThreeActive";
        assert !hasLock || locked in admins, "LockedByAdmin";
        i = i + 1;
      }
    }
  }
}
test sessions [main=Sessions]: { Sessions };
