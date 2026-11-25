# rcheck

For a model with `entity` definitions, `rcheck generate <model> glossary`
exports the checked names and definitions as JSON. The IR retains these
definitions. The [bank model](../../fixtures/rcheck/bank.model) generates
both language bindings. Its [snapshot sequence](../../fixtures/rcheck/bank.trace.json)
can be replayed against the Rust and TypeScript drivers using
`just rcheck-test` and `just rcheck-typescript`.

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

## Dead behavior

```sh
rcheck dead-code model.model
rcheck dead-code model.model --max-states 200000
```

The reference evaluator checks whether each action's guard holds for some
argument binding in a reachable state. JSON reports `coverage` and a status
for each action. A witness makes it `reachable`. A complete, nonempty
search with no witness makes it `unreachable`. A search that runs out of
budget or leaves the integer scope reports unseen actions as `unknown`.
A model without initial states reports `empty`. The budget counts
configuration candidates, initial candidates, and reached states.
`Declarations` checks multiplicity violations in updates separately.

Exit status is 0 for complete coverage with no unreachable action, 1 for
unreachable actions, 2 for invalid input (including IR with adapter
omissions), and 3 for unknown or empty coverage.

## Lean snapshot oracle

```sh
rcheck lean-oracle fixtures/rcheck/bank.model formal/rmodel/Bank.lean
```

The reference evaluator must finish a nonempty search within the model
scope and the `--max-states` budget. rcheck runs the linked Lean file with
`lean --run`, passing a JSON array of `{ "config": ..., "state": ... }`
observations on stdin. The values use the same encoding as the generated
Rust and TypeScript bindings. The file's `main` returns a JSON array with
one boolean per observation in input order. [The bank oracle](../../formal/rmodel/Bank.lean)
defines an independent Lean `Prop` for balances and evaluates it with `decide`.

A false result returns its configuration and state and exits 1. Empty
or incomplete exploration exits 3 before invoking Lean. A model declaration
violation, Lean failure, or invalid response exits 2. A successful result
covers the finite set of observations. Lean theorems have the scope of their
own statements. Run `just rcheck-lean-oracle` with Lean on PATH to exercise
the normal bank, the weakened model, and invalid oracle responses.

## Implementations

`rcheck generate <model> rust|typescript|schema|states` writes bindings for
an implementation: a Rust trait with one method per action, TypeScript types
with a JSON Lines driver, and JSON Schema of the configuration, the state and
the commands. `rcheck::conform` replays the implementation along the
reference semantics and reports the shortest command sequence where it
differs.
