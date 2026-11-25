using Bank;
using Koine.Runtime;
using Orders;
using Permissions;
using Service;
using Sessions;

static void Expect(bool condition, string observation)
{
    if (!condition) throw new InvalidOperationException(observation);
}

static void Reject(Action step, string observation)
{
    try
    {
        step();
    }
    catch (DomainInvariantViolationException)
    {
        return;
    }
    throw new InvalidOperationException(observation);
}

// Permission configuration and state are projected to user-document pairs.
var u0 = new UserId("u0");
var u1 = new UserId("u1");
var d0 = new DocumentId("d0");
var d1 = new DocumentId("d1");
var r0 = new RoleId("r0");
var docRoles = new[] { new RoleId("r0") };
Reject(() => Access.Start(u0, d1, [r0], docRoles, true, false), "illegal permission premise");
var ungranted = Access.Start(u1, d0, [], docRoles, false, false);
Reject(ungranted.Read, "read without grant");
Reject(ungranted.Close, "close without open");
var first = Access.Start(u0, d0, [r0], docRoles, false, false);
var second = Access.Start(u0, d1, [r0], docRoles, false, false);
Expect(!first.Open && !second.Open, "initial open relation is empty");
first.Read();
second.Read();
first.Close();
Expect(!first.Open && second.Open, "close changes only the selected pair");
var secret = Access.Start(u0, d1, [r0], docRoles, true, true);
secret.Read();
Expect(secret.Open && secret.Cleared, "cleared user can open a secret document");
Console.WriteLine("permissions: premise, guards and secret-read witness");

var a0 = Account.Start(new AccountId("a0"));
var a1 = Account.Start(new AccountId("a1"));
Expect(a0.Balance == 0 && a1.Balance == 0, "initial balances");
a0.Deposit(3);
Expect(a0.Balance == 3 && a1.Balance == 0, "Rich in one step, independent accounts");
a0.Withdraw(2);
Reject(() => a0.Deposit(4), "deposit above cap");
Reject(() => a0.Withdraw(2), "overdraw");
Expect(a0.Balance == 1, "rejected commands leave the balance alone");
a0.Deposit(3);
a1.Deposit(4);
Expect(a0.Balance + a1.Balance == 8, "both balances can reach four");
Console.WriteLine("bank: initial balances, Rich witness and transaction guards");

var order = Order.Start(new OrderId("o0"));
Expect(order.Status == Status.Draft, "order starts as Draft");
order.Place();
Reject(order.Place, "cannot place a placed order");
order.Ship(Carrier.Courier);
Reject(() => order.Cancel(Reason.Fraud), "cannot cancel a shipped order");
Expect(order.Status == Status.Shipped && order.Tracking == Carrier.Courier && order.Reason is null,
    "terminal shipped order retains its payload");
var fraud = Order.Start(new OrderId("o1"));
fraud.Cancel(Reason.Fraud);
Expect(fraud.Status == Status.Cancelled && fraud.Reason == Reason.Fraud && fraud.Tracking is null,
    "FraudCancelled in one step");
Console.WriteLine("order: terminal shipping payload and fraud witness");

Reject(() => Session.Start(false, false, false), "admin set must be nonempty");
var session = Session.Start(true, false, false);
Expect(session.ActiveCount == 0 && session.Locked is null, "initial session state");
session.Login(UserSlot.U0);
session.Login(UserSlot.U1);
Reject(() => session.Lock(UserSlot.U1), "non-admin cannot lock");
session.Lock(UserSlot.U0);
Expect(session.ActiveCount == 2 && session.Locked == UserSlot.U0, "Full witness");
Reject(() => session.Login(UserSlot.U2), "capacity guard");
session.Logout(UserSlot.U0);
Expect(!session.Active0 && session.Locked == UserSlot.U0, "counterexample to LockedIsActive");
session.Login(UserSlot.U2);
Reject(() => session.Unlock(UserSlot.U1), "only lock owner can unlock");
session.Unlock(UserSlot.U0);
Expect(session.ActiveCount == 2 && session.Locked is null, "unlock restores absence");
Console.WriteLine("sessions: Full witness, capacity guard and LockedIsActive counterexample");

var c0 = Client.Start(new ClientId("c0"));
var c1 = Client.Start(new ClientId("c1"));
Reject(c1.Serve, "cannot serve before a request");
c0.Request();
c1.Request();
c1.Serve();
Expect(c0.Waiting && !c0.Served && !c1.Waiting && c1.Served, "the two clients evolve independently");
Reject(c1.Request, "served client cannot request twice");
Expect(!c1.Waiting && c1.Served, "rejected request leaves the client served");
Console.WriteLine("service: guarded transitions and concrete disjointness example");
