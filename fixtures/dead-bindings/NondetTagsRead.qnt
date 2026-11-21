module NondetTagsRead {
  type Event = Request(int) | Reply({ id: int, ok: bool }) | Failure(int) | Cancel(int) | Reset
  pure val Ids = 1.to(12)
  pure val Tags = Set("request", "reply", "failure", "cancel", "reset", "idle1", "idle2", "idle3", "idle4")
  var pending: Set[int]
  var done: int
  var last: str
  pure def eventsWithTag(tag: str): Set[Event] =
    if (tag == "request") Ids.map(i => Request(i))
    else if (tag == "reply") Ids.map(i => Set(true, false).map(ok => Reply({ id: i, ok: ok }))).flatten()
    else if (tag == "failure") Ids.map(i => Failure(i))
    else if (tag == "cancel") Ids.map(i => Cancel(i))
    else Set(Reset)
  pure def ownedEvents(p: Set[int], failing: bool): Set[Event] =
    if (failing) p.map(i => Failure(i)) else p.map(i => Reply({ id: i, ok: true })).union(Set(Reset))
  pure def apply(p: Set[int], d: int, e: Event): (Set[int], int) = match e {
    | Request(i) => if (p.size() < 3) (p.union(Set(i)), d) else (p, d)
    | Reply(r) => if (p.contains(r.id) and r.ok) (p.exclude(Set(r.id)), (d + 1) % 4) else (p, d)
    | Failure(i) => (p.exclude(Set(i)), d)
    | Cancel(i) => (p.exclude(Set(i)), d)
    | Reset => (Set(), d)
  }
  pure def label(e: Event): str = match e {
    | Request(_) => "request" | Reply(_) => "reply" | Failure(_) => "failure" | Cancel(_) => "cancel" | Reset => "reset"
  }
  action init = all { pending' = Set(), done' = 0, last' = "init" }
  action step = {
    nondet preferOwned = Set(true, false).oneOf()
    nondet preferFailure = Set(true, false).oneOf()
    nondet tag = Tags.oneOf()
    nondet e = (if (preferOwned) ownedEvents(pending, preferFailure and pending != Set()) else eventsWithTag(tag)).oneOf()
    val after = apply(pending, done, e)
    all { preferOwned == preferOwned, preferFailure == preferFailure, tag == tag, pending' = after._1, done' = after._2, last' = label(e) }
  }
  val safety = pending.size() <= 3 and done < 4
  val broken = done < 3
}
