enum User { U0, U1 }
enum Role { R0, R1 }
enum Doc { D0, D1 }

machine Permissions {
  var grants: map[User, set[Role]];
  var reads: map[Role, set[Doc]];
  var secret: set[Doc];
  var cleared: set[User];
  var opened: set[(User, Doc)];

  fun canRead(u: User, d: Doc): bool {
    var role: Role;
    foreach (role in grants[u]) {
      if (d in reads[role]) {
        return true;
      }
    }
    return false;
  }

  fun readDoc(u: User, d: Doc) {
    if (canRead(u, d)) {
      opened += ((u, d));
    }
  }

  fun closeDoc(u: User, d: Doc) {
    if ((u, d) in opened) {
      opened -= ((u, d));
    }
  }

  fun configure(users: set[User], roles: set[Role], docs: set[Doc]) {
    var u: User;
    var r: Role;
    var d: Doc;
    var userRoles: set[Role];
    var roleDocs: set[Doc];
    var noRoles: set[Role];
    var noDocs: set[Doc];
    var noUsers: set[User];
    secret = noDocs;
    cleared = noUsers;
    foreach (d in docs) {
      if (choose()) {
        secret += (d);
      }
    }
    foreach (u in users) {
      userRoles = noRoles;
      foreach (r in roles) {
        if (choose()) {
          userRoles += (r);
        }
      }
      grants[u] = userRoles;
    }
    foreach (r in roles) {
      roleDocs = noDocs;
      foreach (d in docs) {
        if (choose()) {
          roleDocs += (d);
        }
      }
      reads[r] = roleDocs;
    }
    // Generate only configurations satisfying the premise.
    foreach (u in users) {
      foreach (d in secret) {
        if (canRead(u, d)) {
          cleared += (u);
        }
      }
      if (choose()) {
        cleared += (u);
      }
    }
  }

  start state Init {
    entry {
      var users: set[User];
      var roles: set[Role];
      var docs: set[Doc];
      var userRoles: set[Role];
      var roleDocs: set[Doc];
      var u: User;
      var d: Doc;
      var edge: (User, Doc);
      var i: int;

      users += (U0);
      users += (U1);
      roles += (R0);
      roles += (R1);
      docs += (D0);
      docs += (D1);

      userRoles += (R0);
      roleDocs += (D0);
      grants[U0] = userRoles;
      reads[R0] = roleDocs;
      secret += (D0);
      cleared += (U0);
      readDoc(U0, D0);
      assert (U0, D0) in opened && D0 in secret, "SecretRead witness";
      closeDoc(U0, D0);
      assert sizeof(opened) == 0;

      configure(users, roles, docs);
      while (i < 12) {
        u = choose(users);
        d = choose(docs);
        if (choose()) {
          readDoc(u, d);
        } else {
          closeDoc(u, d);
        }
        foreach (edge in opened) {
          assert !(edge.1 in secret) || edge.0 in cleared, "NoLeak / UnclearedSecret";
          assert canRead(edge.0, edge.1), "OnlyGranted";
        }
        i = i + 1;
      }
    }
  }
}
test permissions [main=Permissions]: { Permissions };
