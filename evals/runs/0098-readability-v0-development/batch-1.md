You are reading models written in a specification language you may not have seen. Read each model and answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Model 307

```
module G307 {
  type S0
  type S1
  const r0: Set[S0]  // at least one element
  const r1: S1 -> Set[S0]  // each key relates to at least one value
  var v2: Set[S0]  // at most one element
  assume S1.exclude(S1).exclude(r1.image(S0)).subseteq(r0)
  action init = all { or { tuples(S0, S1).image(S0).nonEmpty(), tuples(v2, S0).compose(r1) == tuples(S0, Set()) } }
  action a0(p0_0: S1, p0_1: S0) = all {
    Set().size() == 1,
    r1.exclude(r1).preimage(r1.preimage(Set(p0_0))).subseteq(r1.image(tuples(r0, S0).preimage(S1))),
    v2' = tuples(S1, Set(p0_0)).image(r1.preimage(v2)),
  }
  action a1(p1_0: S0, p1_1: S0) = all {
    v2' = Set(),
  }
  action a2() = all {
    v2' = S1,
  }
  invariant Inv = tuples(S1, S1).compose(r1).preimage(tuples(S0, r0).preimage(v2)).nonEmpty()
  reachable Goal = S0.exists(q1 => S0.intersect(v2.exclude(Set())) != S1.union(Set()))
  check { S0 = 3, S1 = 2, steps = 6 }
}
```

Questions:
- `307.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `307.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `307.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `307.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no
- `307.action.a2`: Can action a2 take a step from some reachable state? Options: yes | no

## Model 308

```
module G308 {
  type S0
  type S1
  type E = Ea | Eb
  var v0: E -> Set[E]
  action init = all { and { not(S1.subseteq(Set())), v0.exclude(tuples(S1, Set(Ea))) == tuples(E, S1).compose(tuples(Set(Ea), S1)) } }
  action a0(p0_0: S1) = all {
    v0.preimage(E).contains(p0_0),
    or { v0.image(Set(Eb)).size() <= 2, Set(Ea).union(Set(Ea)).size() <= 0 },
    v0' = tuples(S0, Set(Ea)),
  }
  action a1(p1_0: E) = all {
    and { Set(p1_0).union(S0).size() > 2, v0.preimage(S0).size() != 3 },
    v0' = v0,
  }
  invariant Inv = E != v0.preimage(S0)
  reachable Goal = (S1.forall(q3 => tuples(S0, S1).image(S0.intersect(Set(q3))) == v0.compose(v0).preimage(E.exclude(Set(q3))))).implies(or { (not(v0.exclude(v0).preimage(tuples(E, E).image(Set(Eb))).subseteq(E))).implies(tuples(Set(), Set(Ea)) == v0.compose(v0)), Set(Eb).nonEmpty() })
  check { S0 = 1, S1 = 2, steps = 6 }
}
```

Questions:
- `308.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `308.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `308.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `308.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no

## Model 310

```
module G310 {
  type S0
  type E = Ea | Eb
  const r0: Set[S0]  // at most one element
  var v1: Set[S0]  // at least one element
  action init = all { (Set(Ea) == v1).implies(v1.contains(Eb)) }
  action a0() = all {
    not(Set().exclude(Set()).size() <= 3),
    v1' = Ea,
  }
  action a1() = all {
    E.intersect(E).size() > 1,
    and { r0.size() <= 2, r0.exclude(Set().intersect(r0)) == v1 },
    v1' = Eb,
  }
  action a2() = all {
    and { Set(Ea).nonEmpty(), Set(Ea).union(v1).size() <= 3 },
    v1' = Eb,
  }
  invariant Inv = and { v1.exclude(S0).size() <= 0, S0.size() > 1 }
  reachable Goal = or { Set(Eb).size() <= 0, S0.forall(q3 => v1.nonEmpty()) }
  check { S0 = 1, steps = 6 }
}
```

Questions:
- `310.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `310.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `310.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `310.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no
- `310.action.a2`: Can action a2 take a step from some reachable state? Options: yes | no

## Model 311

```
module G311 {
  type S0
  const r0: Set[S0]  // at least one element
  const r1: Set[S0]
  var v2: Set[S0]  // at most one element
  var v3: Set[S0]
  assume S0.forall(q1 => r0.nonEmpty())
  action init = all { and { r1 != v3, r1.size() == 1 } }
  action a0() = all {
    r0.union(S0).nonEmpty(),
    and { not(S0.subseteq(r0)), S0.nonEmpty() },
    v3' = v3,
  }
  invariant Inv = or { and { and { r1.nonEmpty(), S0 == S0.exclude(S0.intersect(r1)) }, or { v3.size() == 0, S0.exclude(r1).size() == 1 } }, (S0.size() < 1).implies(and { r1 == S0, r1 != v3 }) }
  reachable Goal = (r0.nonEmpty()).implies(not(v2.intersect(r1).subseteq(S0)))
  check { S0 = 2, steps = 6 }
}
```

Questions:
- `311.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `311.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `311.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no

## Model 312

```
module G312 {
  type S0
  type E = Ea | Eb
  var v0: Set[S0]  // at most one element
  action init = all { E.forall(q1 => Set(Eb).exclude(v0).size() <= 1) }
  action a0(p0_0: E) = all {
    not(S0.union(Set(Eb)).exclude(Set(p0_0)) == S0),
    Set(Ea) == S0,
    v0' = E,
  }
  invariant Inv = (Set(Eb).size() >= 1).implies(v0.nonEmpty())
  reachable Goal = v0.nonEmpty()
  check { S0 = 1, steps = 6 }
}
```

Questions:
- `312.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `312.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `312.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
