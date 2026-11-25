enum Order { O0, O1 }
enum Carrier { Post, Courier }
enum Reason { Customer, Fraud }
enum Tag { Draft, Placed, Shipped, Cancelled }
// A tag plus a product of payload fields substitutes for a payload-bearing sum.
type Status = (tag: Tag, tracking: Carrier, reason: Reason);

machine Orders {
  var status: map[Order, Status];

  fun place(o: Order) {
    if (status[o].tag == Draft) {
      status[o] = (tag = Placed, tracking = Post, reason = Customer);
    }
  }

  fun ship(o: Order, tracking: Carrier) {
    if (status[o].tag == Placed) {
      status[o] = (tag = Shipped, tracking = tracking, reason = Customer);
    }
  }

  fun cancel(o: Order, reason: Reason) {
    if (status[o].tag == Draft || status[o].tag == Placed) {
      status[o] = (tag = Cancelled, tracking = Post, reason = reason);
    }
  }

  start state Init {
    entry {
      var orders: set[Order];
      var carriers: set[Carrier];
      var reasons: set[Reason];
      var o: Order;
      var before: Status;
      var action: int;
      var i: int;
      orders += (O0);
      orders += (O1);
      carriers += (Post);
      carriers += (Courier);
      reasons += (Customer);
      reasons += (Fraud);
      foreach (o in orders) {
        status[o] = (tag = Draft, tracking = Post, reason = Customer);
      }
      cancel(O0, Fraud);
      assert status[O0].tag == Cancelled && status[O0].reason == Fraud, "FraudCancelled witness";
      status[O0] = (tag = Draft, tracking = Post, reason = Customer);

      while (i < 12) {
        o = choose(orders);
        before = status[o];
        action = choose(3);
        if (action == 0) {
          place(o);
        } else if (action == 1) {
          ship(o, choose(carriers));
        } else {
          cancel(o, choose(reasons));
        }
        // The enum alone excludes two tags, but mismatched payload fields are representable.
        assert !(status[o].tag == Shipped && status[o].tag == Cancelled), "ShippedCancelled";
        assert before.tag != Shipped || before == status[o], "ShippedStays";
        i = i + 1;
      }
    }
  }
}
test order [main=Orders]: { Orders };
