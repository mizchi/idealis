You are reading models written in a specification language you may not have seen. Read each model and answer every question by reasoning about the text alone; do not run tools or look anything up. Reply with one JSON object that maps each question id to exactly one of the options listed for it, and nothing else.

## Model 300

```
module G300 {
  type S0
  const r0: Set[S0]  // at most one element
  const r1: Set[S0]
  var v2: S0 -> Set[S0]
  var v3: S0 -> Set[S0]
  assume and { not(S0.intersect(S0).size() == 1), and { r1.intersect(r0).nonEmpty(), r0.subseteq(S0) } }
  action init = all { tuples(r0, r1).exclude(v3).image(r0.exclude(S0)) != v3.preimage(tuples(r1, r0).preimage(r0)) }
  action a0(p0_0: S0) = all {
    r0.union(v2.preimage(r0)).contains(p0_0),
    not(v3.preimage(Set(p0_0).union(r0)).isEmpty()),
    v2' = v2,
  }
  action a1(p1_0: S0, p1_1: S0) = all {
    or { v2.image(r1).nonEmpty(), v2.intersect(v3).subseteq(tuples(Set(p1_1), Set(p1_1))) },
    v2' = v2,
  }
  invariant Inv = and { not(tuples(r1, r1).image(r0).nonEmpty()), or { r1.nonEmpty(), and { S0.intersect(Set()).nonEmpty(), not(r0.subseteq(v3.preimage(r1).intersect(r1.intersect(r0)))) } } }
  reachable Goal = not(not(not(v3.subseteq(v2.compose(v3)))))
  check { S0 = 3, steps = 6 }
}
```

Questions:
- `300.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `300.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `300.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `300.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no

## Model 301

```
module G301 {
  type S0
  type S1
  var v0: S1 -> Set[S1]  // each key relates to exactly one value
  action init = all { (v0.compose(tuples(S1, S1)).subseteq(tuples(S1, S0))).implies(v0.image(S0).nonEmpty()) }
  action a0(p0_0: S0) = all {
    (tuples(S0, S1).image(S1).nonEmpty()).implies(Set().size() < 1),
    v0' = v0.exclude(v0).compose(v0),
  }
  invariant Inv = (tuples(S1, S0).compose(v0).image(S0).size() == 1).implies(S0.nonEmpty())
  reachable Goal = or { S1.exists(q1 => or { S1.subseteq(tuples(S0, S1).image(S0).union(v0.preimage(S1))), S0 == tuples(Set(q1), S0).union(tuples(S0, S0)).preimage(v0.preimage(S1)) }), v0.subseteq(tuples(S0, S1)) }
  check { S0 = 3, S1 = 3, steps = 6 }
}
```

Questions:
- `301.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `301.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `301.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no

## Model 302

```
module G302 {
  type S0
  type S1
  type E = Ea | Eb
  const r0: Set[S1]  // exactly one element
  var v1: E -> Set[S1]  // each key relates to at most one value
  var v2: Set[S1]  // exactly one element
  assume or { E.nonEmpty(), Set(Eb).nonEmpty() }
  action init = all { not(v1.preimage(v2).nonEmpty()) }
  action a0() = all {
    and { v1 == tuples(v2, v2), v1.union(v1).preimage(r0).size() <= 1 },
    v1.subseteq(v1.intersect(v1)),
    v1' = v1,
  }
  action a1(p1_0: S0, p1_1: S1) = all {
    not(Set().size() == 2),
    Set(Eb) != tuples(Set(Eb), Set(Ea)).image(Set(Ea)).intersect(v1.preimage(Set(Eb))),
    v2' = v1.image(Set(p1_0)),
  }
  action a2(p2_0: S1) = all {
    S1.exists(q1 => S1.subseteq(tuples(Set(q1), S1).union(tuples(r0, r0)).preimage(S0))),
    E.forall(q2 => tuples(Set(Eb), Set(q2)).image(r0).size() == 2),
    v2' = v1.preimage(Set(Eb).exclude(Set(p2_0))),
  }
  invariant Inv = v1.preimage(v1.image(S1)).isEmpty()
  reachable Goal = not(tuples(E, S0).image(v1.image(S0)).size() == 1)
  check { S0 = 2, S1 = 2, steps = 6 }
}
```

Questions:
- `302.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `302.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `302.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `302.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no
- `302.action.a2`: Can action a2 take a step from some reachable state? Options: yes | no

## Model 304

```
module G304 {
  type S0
  const r0: Set[S0]
  var v1: S0 -> Set[S0]
  var v2: Set[S0]
  action init = all { or { v1.preimage(S0).nonEmpty(), not(tuples(r0, S0).preimage(r0).subseteq(v1.image(v2))) } }
  action a0(p0_0: S0, p0_1: S0) = all {
    and { v2.exclude(Set(p0_0)).nonEmpty(), r0.intersect(S0).size() > 1 },
    v1' = tuples(r0, Set(p0_0)).compose(v1).exclude(tuples(Set(), Set(p0_0))),
  }
  action a1() = all {
    v1' = v1,
  }
  action a2() = all {
    v2' = S0,
  }
  invariant Inv = not(v1.exclude(v1) == tuples(v2, r0))
  reachable Goal = (and { not(v1.compose(v1).image(v2) == v1.compose(v1).image(tuples(r0, S0).preimage(S0))), (v1.compose(tuples(S0, v2)).image(v1.image(v2)).isEmpty()).implies(Set().exclude(v2).size() <= 1) }).implies(not(or { S0.exclude(S0).nonEmpty(), tuples(v2, S0).compose(v1).subseteq(v1.compose(v1)) }))
  check { S0 = 2, steps = 6 }
}
```

Questions:
- `304.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `304.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `304.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `304.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no
- `304.action.a2`: Can action a2 take a step from some reachable state? Options: yes | no

## Model 306

```
module G306 {
  type S0
  type E = Ea | Eb
  const r0: Set[E]  // at least one element
  var v1: Set[S0]  // at least one element
  action init = all { (v1.size() < 3).implies(Set(Eb).exclude(v1).size() >= 2) }
  action a0(p0_0: E, p0_1: E) = all {
    v1' = p0_1,
  }
  action a1() = all {
    not(r0.size() == 1),
    and { E.size() > 1, E.size() >= 2 },
    v1' = Eb,
  }
  action a2() = all {
    not(Set(Ea).nonEmpty()),
    v1' = E,
  }
  invariant Inv = and { S0.exists(q3 => (not(E.subseteq(v1.exclude(Set(q3))))).implies(Set(q3) == E)), not(not(v1.contains(Ea))) }
  reachable Goal = v1.union(Set(Eb)).nonEmpty()
  check { S0 = 3, steps = 6 }
}
```

Questions:
- `306.Inv`: Does invariant Inv hold in every reachable state, for every allowed value of the constants? Options: holds | violated | empty (no initial state exists)
- `306.Goal`: Is a state satisfying Goal reachable, for some allowed value of the constants? Options: reachable | unreachable | empty (no initial state exists)
- `306.action.a0`: Can action a0 take a step from some reachable state? Options: yes | no
- `306.action.a1`: Can action a1 take a step from some reachable state? Options: yes | no
- `306.action.a2`: Can action a2 take a step from some reachable state? Options: yes | no
