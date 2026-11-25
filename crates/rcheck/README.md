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

IR files from adapters must describe a complete model to be checked by this
CLI. If an adapter lists missing source parts in `omitted`, rcheck reports
them and exits before running any checker.

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

## Split sources and boundary properties

A `*.rmodel.json` file lists ordered `fragments`, for example
`{"fragments":["domain.model","withdrawal.model"]}`. Paths are relative
to the manifest. The fragments are parsed together with one model name and
one namespace. This is an input format for rcheck, with the usual rmodel
syntax in each fragment. See the [withdrawal](../../fixtures/rcheck/linked/withdrawal/project.rmodel.json)
and [Raft election](../../fixtures/rcheck/linked/raft/project.rmodel.json)
projects. The withdrawal uses a record for money and an enum for outcomes.
A request can be accepted and later complete with `InsufficientFunds`.

```sh
model=fixtures/rcheck/linked/withdrawal/project.rmodel.json
rcheck "$model" --checker reference
rcheck generate "$model" typescript > bindings.ts
rcheck generate "$model" pbt-cases > cases.json
rcheck generate "$model" pbt-typescript > boundary.ts
just rcheck-boundaries
```

`pbt-cases` emits versioned JSON. The reference evaluator first checks that
it can completely explore the finite model. For each configuration, rcheck
selects one witness pair per action and boundary kind:

- `integer` changes one integer argument or record field by one in the same state.
- `transition` tests the same command on both sides of one reachable state transition.
- `continuation` tests the same command after two accepted inputs that differ
  by one integer. This exposes the exact-balance boundary in the withdrawal model.

Each pair shows opposite acceptance results. Each example includes the
commands leading to the boundary and the expected snapshot after the final
command, including cases where it was rejected. Generation fails for
incomplete or invalid exploration, ambiguous initial states, or an empty
corpus. Extraction samples the earliest witness for each action and kind
within the finite scope. TypeScript generation requires the model’s integer
scope to fit JavaScript’s safe integers.

`pbt-typescript` embeds the same cases in a Node.js module with a fast-check
property. Pass `{ decodeConfig, create }` to `boundaryProperty`. The property
passes the decoded configuration to `create`. The factory returns `apply`,
which accepts the generated command JSON and returns a boolean, and
`observe`, which returns the implementation's state as JSON. The property samples and shrinks lists
of pairs, verifies that prefix commands are accepted, and compares both the
final acceptance and snapshot. The test can force every pair to be exercised
by passing `examples: [[boundaryCases.pairs]]` to fast-check. See the
[withdrawal test](../../tests/rcheck/withdrawal.pbt.test.ts) for an adapter
using the generated bindings and a replay with a stored seed and path.

### Generated Go interface and fuzz test

Generate both files in the same Go package. For the withdrawal example the
package is `withdrawal`.

```sh
rcheck generate "$model" go > bindings.go
rcheck generate "$model" pbt-go > boundary_generated_test.go
```

Implement the interface in `bindings.go` and register a factory in your
test. The generated helper calls the factory with a typed `Config`.

```go
func FuzzWithdrawalBoundaries(f *testing.F) {
    CheckBoundaryCases(f, func(config Config) Withdrawal {
        return newAccount(config)
    })
}
```

`go test` replays every pair as a fuzz seed. In fuzz mode the input selects
a pair from the same corpus. Each example uses an independent instance and
checks the acceptance and snapshot of the final command. Run
`just rcheck-go` to run the [withdrawal implementation and fault control](../../tests/rcheck/go/withdrawal/withdrawal_test.go).

## Validate before running a command

Use a checked model to create a Rust `Validator`. Start a session with
the supplied configuration and initial state. The session checks the wire
types, finite scope, facts, initial predicates and invariants.

```rust
let validator = rcheck::conform::Validator::new(&model)?;
let mut session = validator.start(&config, &initial_state)?;
let predicted = session.validate(&command)?;
let observed = session.dispatch(&command, |input| domain.apply(input))?;
assert_eq!(observed, predicted);
```

`validate` returns the expected next snapshot and has no domain effects.
`dispatch` calls the domain only if the guard holds and the predicted
state satisfies the model. It checks the actual acceptance and state
returned by the domain. A failure or divergent response leaves that
session unusable. The model check cannot undo effects from a domain that
has already started.

Go, TypeScript and other processes can use the pure JSON interface.
Each request supplies a configuration, an initial snapshot and an
optional prefix of accepted commands. Supply the prefix for the actual
domain instance you intend to call.

```sh
rcheck validate fixtures/rcheck/nat-account.model - <<'JSON'
{
  "config": {"limit": 3},
  "initial": {"balance": {"units": 3}},
  "prefix": [{"kind": "withdraw", "amount": {"units": 2}}],
  "command": {"kind": "withdraw", "amount": {"units": 1}}
}
JSON
```

This returns `{"state":{"balance":{"units":0}}}`. Invalid inputs, disabled
commands and model violations exit 2 with a diagnostic. The Rust API
distinguishes these cases in `ValidationError`.

## Structured values and progress checks

```text
sequence Journal = Entry, max = 2
variant Outcome = Pending | Paid(Amount) | Rejected(Reason)
```

Use `Journal[A, B]`, `append(log, entry)`, `len(log)` and `at(log, index)`.
Indices start at zero. An out-of-range read returns the empty set. Appending
past the declared maximum leaves the scope when the result is stored.
The wire shape `{"$sequence":["A","B"]}` preserves order and duplicates.

Construct a variant with `Paid(amount)` or `Pending()`. Test the tag with
`is(result, Paid)` and read its payload with `payload(result, Paid)`.
A different tag yields an empty set. The wire shape is
`{"tag":"Paid","value":{"units":2}}` or `{"tag":"Pending"}`.
Rust, Go, TypeScript and MoonBit generate typed values and codecs.

```text
fairness weak serve
eventually FirstService { some served }
leads_to EachRequest(c: Client) { c in waiting } { c in served }
leads_to WithoutFairness(c: Client) with {} { c in waiting } { c in served }
```

Fairness over a parameterized action ranges over the action as a whole.
`with { weak serve }` or `with {}` overrides the defaults for one question.
`eventually` requires a predicate to become true. `leads_to` requires a
response each time its trigger holds. The explicit checker answers temporal
questions, with `lasso` and `loopStart` on a temporal counterexample. Safety
and reachability use the underlying transition system.

Run `just rcheck-structured` for model checks, `just rcheck-adapters` for Pkl,
and `just rmodel-structured-tlc .artifacts/structured-replay` for TLC.

## Checking adjacent states

```text
transition ShippedStays {
  is(before(status), Shipped) implies {
    is(after(status), Shipped)
    payload(before(status), Shipped) = payload(after(status), Shipped)
  }
}
```

A `transition` property checks both snapshots of each enabled model
operation with valid states. This includes explicitly enabled no-ops.
Implicit waiting is outside the operation relation. An initial state with
no enabled operations has no step to violate the property. The unqualified
expression and `before(expr)` read the previous snapshot. `after(expr)`
and the experimental postfix prime read the resulting snapshot.
The same property is checked by the runtime validator before dispatch.

`just rcheck-transition` runs the regression tests.
`just rmodel-transition-tlc .artifacts/transition-replay` checks the
lowered models with TLC. The reference and explicit checkers support
these properties. The SAT backend reports them as unsupported.
