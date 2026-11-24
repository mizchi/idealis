# rcheck

One entry point for the questions of an [rmodel](../rmodel) model: its
`invariant` and `reachable` checks and the built-in `Declarations`
multiplicity check. Each registered checker declares, per question, whether
it answers completely, within a bound, or not at all:

| checker     | answers                                                 | coverage                                                     |
| ----------- | ------------------------------------------------------- | ------------------------------------------------------------ |
| `reference` | the reference evaluator, the semantics' source of truth | complete, when every relation has at most 20 possible tuples |
| `explicit`  | rtlc on the TLA+ lowering                               | complete                                                     |
| `sat`       | ralloy's SAT on the Alloy lowering                      | bounded by the model's `steps`                               |

```sh
rcheck model.model                    # auto: the first complete checker
rcheck model.model --checker all      # every checker, and whether they disagree
rcheck model.model --checker sat --question Safe
```

Answers are `holds`, `violated`, `reachable`, `unreachable`, `empty`,
`unknown` (a budget ran out) or `error`, with the coverage and, for a
violation or a reachable state, the shortest depth. A bounded `holds` is not
a proof, `empty` is not a pass, and `unknown` is never read as either. The
command exits 1 when two definite answers disagree.
