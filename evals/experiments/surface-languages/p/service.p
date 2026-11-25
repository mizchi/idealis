enum Client { C0, C1 }
event eWaitingChanged: int;

machine Service {
  var waiting: set[Client];
  var served: set[Client];

  fun request(c: Client) {
    if (!(c in waiting) && !(c in served)) {
      waiting += (c);
      announce eWaitingChanged, sizeof(waiting);
    }
  }

  fun serve(c: Client) {
    if (c in waiting) {
      waiting -= (c);
      served += (c);
      announce eWaitingChanged, sizeof(waiting);
    }
  }

  fun disjoint() {
    var c: Client;
    foreach (c in waiting) {
      assert !(c in served), "Disjoint";
    }
  }

  start state Init {
    entry {
      var clients: set[Client];
      var c: Client;
      var i: int;
      clients += (C0);
      clients += (C1);

      while (i < 12) {
        c = choose(clients);
        if (choose()) {
          request(c);
        } else {
          serve(c);
        }
        disjoint();
        i = i + 1;
      }

      // The checker tests liveness against a finite environment that drains
      // pending requests. P has no action-level weak-fairness declaration here.
      while (sizeof(waiting) > 0) {
        serve(choose(waiting));
        disjoint();
      }
    }
  }
}

spec EventuallyServed observes eWaitingChanged {
  start cold state Empty {
    on eWaitingChanged goto Waiting;
  }

  hot state Waiting {
    on eWaitingChanged do (count: int) {
      if (count == 0) {
        goto Empty;
      }
    }
  }
}

test service [main=Service]: assert EventuallyServed in { Service };
