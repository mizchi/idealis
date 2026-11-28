# idealis-check

For a model with `entity` definitions, `idealis-check generate <model> glossary`
exports the checked names and default definitions as a flat JSON object.
`idealis-check generate <model> glossary-localized` exports all available definitions
keyed by entity and language, using `default` for untagged text. Both commands
read model source or JSON IR. The [bank model](../../fixtures/rcheck/bank.model) generates
both language bindings. Its [snapshot sequence](../../fixtures/rcheck/bank.trace.json)
can be replayed against the Rust and TypeScript drivers using
`just idealis-check-test` and `just idealis-check-typescript`.

### Bind model declarations to a domain dictionary

A project manifest may declare `"dictionary": "dictionary.ideal"`, relative to
the manifest. The source uses a `vocabulary` block with named `term` entries.
The JSON dictionary format is also accepted, with an `id` and a `terms` array.
Each JSON term has an `id`, a `kind`, and `name` and `definition` maps keyed by language. Use
`term "active-member" for activeMembers` in a source fragment to bind a term to
a declaration. Supported kinds are `entity` (an atom sort or enum), `state`,
`relation` (fixed), `action`, and `check`.

`idealis-check generate PROJECT dictionary` emits the vocabulary and its linked
targets. Unknown IDs and targets, kind mismatches, and duplicate dictionary
IDs fail when loading the project. JSON IR preserves the declaration-to-ID
bindings. The dictionary text is read from the project's file.
[Library and membership example](../../examples/model-patterns/library-membership/README.md)

One entry point for the questions of an [rmodel](../idealis-model) model: its
`invariant` and `reachable` checks and the built-in `Declarations`
multiplicity check. Each registered checker declares, per question, whether
it answers completely, within a bound, or not at all:

| checker     | answers                                                           | coverage                                                     |
| ----------- | ----------------------------------------------------------------- | ------------------------------------------------------------ |
| `reference` | the reference evaluator, the semantics' source of truth           | complete, when every relation has at most 20 possible tuples |
| `explicit`  | rtlc on the TLA+ lowering                                         | complete                                                     |
| `sat`       | ralloy's SAT on the Alloy lowering                                | bounded by the model's `steps`                               |
| `rprob`     | bitset search for finite state, transition and temporal questions | complete within its supported model sizes and expressions    |

```sh
idealis-check --help                          # inputs, commands, and generation targets
idealis-check model.model                    # auto: the first complete checker
idealis-check model.model --checker all      # every checker, and whether they disagree
idealis-check model.model --checker sat --question Safe
idealis-check model.model --checker rprob --question Safe --expect holds
```

Answers are `holds`, `violated`, `reachable`, `unreachable`, `empty`,
`unknown` (incomplete coverage) or `error`, with the coverage and, for a
violation or a reachable state, the shortest depth. A bounded `holds` is not
a proof, `empty` is not a pass, and `unknown` is never read as either. The
command exits 1 when two definite answers disagree.
The SAT bound counts the initial state. A violation after eight operations
requires `scope steps = 9` to fall within the bound.

Rust hosts can call `idealis_check::check_with` with an ordered slice of
`&dyn Checker` values. Include the built-in checkers you want or supply
your own. `auto` selects the first checker that declares complete coverage,
then the first supported checker if none declares it. Each checker can
decline a question or return `unknown` with a budget diagnostic.
Answer metadata is checked against the declared coverage and question.
Comparisons use the bound of each answer, counting its initial state.
`idealis_check::check` and the CLI use the built-in registry.

The kernel JSON IR currently accepts version 1. Incompatible versions
are diagnosed by version before decoding their fields or opcodes. For
supported versions, unknown fields and opcodes are rejected and the
decoded model is type-checked. A frozen early Pkl IR exercises compatibility
with a checked source model and a replayed witness.
[Compatibility fixture](../../fixtures/rcheck/ir-compat/README.md)

## Exchange evidence with an external checker

```sh
idealis-check checker-request fixtures/rcheck/external/panel.model CanOpen > request.json
python3 -B tests/idealis-check/external-panel.py < request.json > reply.json
idealis-check checker-verify request.json reply.json
```

The version 1 request contains the checked JSON `ir`, a `question`
with its name and kind, and the search `budget`. Each reply provides
`version: 1`, the request's `requestId`, and a `verdict`. The identifier
binds the IR, question, and budget. An adapter can return `unsupported`
or `error` with a reason. Other verdicts declare `coverage` as
`{"kind":"complete"}` or `{"kind":"bounded","steps":N}`, including the
initial state in the bound.

A `reachable` or `violated` reply supplies a `witness` with `config`,
`initial`, `commands`, and `finalState`. The verifier replays the commands
and checks that the final state or last edge establishes the requested
result for a finite question.

A temporal `violated` reply additionally supplies `witness.loopStart`, the
zero-based index of the state where the cycle begins. The last command closes
the loop, and `finalState` repeats the state at `loopStart`. Use JSON `null`
for a stuttering edge. A bound of `steps: N` counts the prefix and one copy of
the cycle in state slots, including the initial state and excluding the
repeated closing state. The verifier checks the replay, the fairness of the
repeated cycle, and the violation of the requested `eventually` or `leads_to`
property. The `trace` from `idealis-check witness` has the same witness shape.

Exit 0 means the supplied witness was verified. Reported verdicts such as
`holds`, `unreachable`, `empty`, `unknown` and `unsupported` exit 3. A temporal
witness the verifier cannot finish within `budget.work` also exits 3 with a
`verificationReason`. A failed checker, invalid reply, or invalid witness
exits 2. Coverage remains the adapter's declaration.

The sample Python checker provides a separately implemented explorer for
one Boolean state variable.

## Follow a rule to its checks and back

```sh
idealis-check traceability spec/rmodel-design.json --rule RMODEL-RECEIPT-IMMUTABILITY \
  --document spec/rmodel-design.md
idealis-check traceability spec/rmodel-design.json \
  --check spec/models/checker-comparison.model ReceivedResultStays \
  --document spec/rmodel-design.md
```

Pass an idealis-spec JSON module with a `spec` property, as produced by
`pkl eval -f json`. A handwritten module can omit lists whose Pkl default
is empty. `--rule` runs the rule's claims and linked clauses. `--check`
finds claims by their exact model path and check name in the specification.
Both return the rule text, expected
verdict, and current evidence, including replayable witnesses when found. Claim
model paths are relative to the working directory or the supplied `--root`.
`--document` resolves the heading line in the rendered Markdown and checks its
rule text against the specification.

Exit 0 indicates that every selected claim matched. Exit 1 means a definite
mismatch. Exit 2 reports an invalid input or link. Exit 3 means the rule or check
has no link, or a search was inconclusive. `--max-states` sets a budget per
claim. The result tracks correspondence to named checks and the current model
verdict. Use the idealis-spec review workflow to assess whether a natural-language
sentence and its formal check express the same requirement.

### Find users of a state variable

```sh
dir=examples/model-patterns/library-membership
spec_json=$(mktemp)
pkl eval -f json "$dir/spec.pkl" > "$spec_json"
idealis-check impact "$dir/notification.project.json" knownActiveMembers \
  --spec "$spec_json" --root "$dir"
rm "$spec_json"
```

`impact` accepts model source or a project manifest. Query the linked state
name after port resolution. The report includes source locations for actions
that read or write the state, named derived expressions that read it, and
checks that read it or refer to an affected action through `on` or fairness.
`reads` and `viaActions` distinguish the reasons a check appears.

With `--spec`, the report also lists linked rule IDs and any implementation
and test links declared for the model as a whole. Relative model paths in the
specification use `--root`, which defaults to the current directory. Static
references identify review candidates. Run the relevant checks and replay
against the implementation to establish behavioral impact.

## Check an expected verdict in CI

```sh
idealis-check fixtures/rcheck/autosave.model --question FinishDoesNotEdit --expect holds
```

For SAT, use `--sat-steps N` to set the trace bound for this run. The
bound counts states, including the initial state. Without the flag, the
SAT backend uses a model's legacy `scope steps = N` when present, or
four states by default. SAT reports the effective bound in its coverage
and supports bounds from 1 through 32. For reachability, SAT distinguishes
an empty initial state space from an unreachable goal. It checks for an
initial state with the remaining work budget when the goal has no witness.
If the budget runs out during this check, the result is `unknown`.

`--expect` takes `holds`, `violated`, `reachable`, or `unreachable` and
requires `--question`. The command continues to print the usual JSON
report. Its exit status is:

| Status | Meaning                                                                                                         |
| ------ | --------------------------------------------------------------------------------------------------------------- |
| 0      | At least one definitive answer matches.                                                                         |
| 1      | A definitive answer differs, the model has no initial state in a complete search, or the checkers disagree.     |
| 2      | Invalid input, an unsupported checker selection, or a checker failure.                                          |
| 3      | There is no definitive answer, for example because of a budget limit or a bounded search that found no witness. |

`holds` and `unreachable` require complete coverage. An actual violation
or reachable state can satisfy its expectation within a bound. With
`--checker all`, other results may be inconclusive or unsupported. An
error, disagreement, or different definitive answer still fails the check.
Without `--expect`, use the JSON verdict to interpret a completed command.

## See available checkers before searching

```sh
idealis-check capabilities examples/model-patterns/autosave/autosave.ideal
idealis-check capabilities examples/model-patterns/stream-terminal/split-terminal.ideal
```

Each entry in `questions` names a question and its kind. Each registered
checker reports `supported` with the coverage it can attempt, or
`unsupported` with the same eligibility reason used by normal checks.
Run a supported checker with `idealis-check MODEL --question NAME --checker CHECKER`.

`rprob` in the checker list describes the normal `--checker rprob` check.
The `rprobWitness` field gives the same eligibility for
`idealis-check witness MODEL NAME --checker rprob`, which supplies replay data.
For example, in the split-terminal model rprob can find a finite path to
`ControlAndSecondOvertakeFirst` or check `EndEventuallyFollowsTheSeal`.

`scopeWitness.question` gives the built-in `Scope` diagnostic name.
`scopeWitness.reference` reports support for `idealis-check witness MODEL Scope`.
`scopeWitness.rprob` reports support or the eligibility reason for the same
command with `--checker rprob`. Run either command to find the operation
that would write a value outside the finite scope.

The command loads and type-checks the input and can lower it for eligibility.
It never explores reachable states. Exit 0 means the support report was
produced. Invalid input or an IR with omitted parts exits 2. A supported
checker can still return an empty or incomplete result when run.

## Search budgets and diagnostics

Pass `--max-states N` to set the reference and explicit state budget. The
reference checker counts configurations and initial candidates as well as
reachable states. It binds fixed values from unconditional facts and
initial constraints before enumerating remaining candidates, using the
same initial values as `witness`, `coverage`, and `dead-code`.
Incomplete checks and witnesses can carry a `diagnostics` array.
`budget` names the exhausted resource, `scope_exit` can include the depth
of the shallowest observed exit, and `enumeration_limit` names a relation the
reference checker could not enumerate. `argument_enumeration_limit` names
the operation and sort when one argument is too large to construct. `dead-code` reports the same
reference diagnostics at the top level. A question report also lists
skipped checkers and their reasons in `unsupported`, when applicable.

Finite reference search uses a separate default of 2,000,000 operation
bindings. `witness`, `witness-step` and `dead-code` accept
`--max-arguments N` to change it. Finite `witness --checker rprob` also accepts
this option. Each binding is charged before its
arguments are constructed, including calls rejected after binding and
calls without parameters. For a parameterized operation, the leading
checks that do not read arguments are evaluated once for each state. If
one is false, the operation spends no argument trials in that state.
Exhausting this budget reports
`{"kind":"budget","resource":"arguments"}`. A witness found within the
budget remains available with `complete: false`.

For SAT, the declaration counts are checked against ralloy's limits
before parsing or solving the generated Alloy. For example, a model with
nine fields lists SAT in `unsupported` with the eight-field limit.
Other eligible checkers can still report their results. A named check
with an ineligible checker returns the reason as a command error.

IR files from adapters must describe a complete model to be checked by this
CLI. If an adapter lists missing source parts in `omitted`, idealis-check reports
them and exits before running any checker.

## Witnesses for replay

```sh
idealis-check witness examples/model-patterns/outbox/delivery.ideal StoppedAfterSend
```

A reachable state or violation includes a `trace` with `config`, `initial`,
`commands`, and `finalState` in the same JSON shape used by the runtime adapter.
A violated transition includes the offending operation. The validator will
reject an operation that violates the model when replaying that final step.
A trace can be empty for an initial-state witness.

The reference evaluator stores paths only for this subcommand. It returns a
shortest witness when `complete` is true. The finite witness search narrows
configuration and initial-state candidates using unconditional equalities
and empty-set constraints. With `--max-states N`, an incomplete search can
return a witness it has already found, or `unknown` with a null trace.

Use `idealis-check witness MODEL QUESTION --checker rprob` to record a path from
the rprob bitset search. The option covers its supported state, transition,
declaration, scope and temporal questions. The trace uses the same replay format.
`--max-states` sets the state budget. Finite rprob witnesses can also set
`--max-arguments`. Models with indexed action arguments use the default
argument budget when the limit is omitted. The temporal graph uses the default
rmodel operation-instance budget.

For `idealis-check witness MODEL Scope --checker rprob`, the reference runtime
executes the path found by rprob once to recover all writes of the final
operation in `trace.finalState`, including values outside the scope. A scope
exit sets `complete` to false. Exploration continues through states inside scope.

### Find a step satisfying a predicate

```sh
idealis-check witness-step examples/model-patterns/subscriptions/partition-restart.ideal receive \
  'attempt = Second and before(active) = Third and some before(Third.requested) and unchanged(accepted)'
```

The predicate can refer to the selected action's arguments and the model's
`derive` declarations and use `before`, `after`, and `unchanged`.
Use `no none` to find any invocation.
The reference evaluator searches
transitions whose guard holds and whose next state fits its declarations
and scope. A `reachable` trace includes the matching action as its last
command. A complete search with no match returns `unreachable`. No initial
state returns `empty`. An exhausted budget or a scope exit can leave a
query `unknown`. A trace is guaranteed to be shortest when `complete`
is true. Replay with `validate` also checks the model's declared transition
properties.

For a violated temporal question, either explicit search or rprob can supply a lasso.

```sh
idealis-check witness examples/model-patterns/worker-fairness/competitive.ideal InstanceWeak
idealis-check witness examples/model-patterns/worker-fairness/competitive.ideal InstanceWeak --checker rprob
```

A regular `--checker rprob --question InstanceWeak` check returns the same replay
data as `answers[].lasso`.

The witness adds a zero-based `trace.loopStart` to the same JSON shape.
Run the commands before `loopStart` once, then repeat the remaining commands.
The last edge returns to the loop entry, which is also `finalState`.

idealis-check replays rprob's proposed lasso against the reference evaluator, including
its fairness and temporal conditions. An incomplete graph, scope exit or
declaration violation yields `unknown` for a temporal question.
A `null` command represents a stuttering edge with no runtime operation.

For witness generation, each in-scope combination of action arguments has its
own transition label. The export uses that label to return the chosen
arguments and checks the edge against the reference evaluator. The compiler
allows up to 4,093 concrete action instances in this lowering. The regular
checker still explores the quantified transitions. A finite replay exercises
a chosen number of iterations.

## Finding a pair of indistinguishable snapshots

`observation-pair` selects reachable states satisfying a named `reachable`
condition. Supply state-variable names in `observe` and `distinguish`.
Within each configuration, it finds two states with the same observed
values and different internal values. Each returned trace contains
`config`, `initial`, `commands`, and `finalState`, like a finite witness.

```sh
idealis-check observation-pair examples/model-patterns/object-store/multipart-reconcile.ideal - <<'JSON'
{"at":"LatestObservation","observe":["observed","result","proof"],"distinguish":["versions"]}
JSON
```

A completed search returns a shortest pair by total command count, or
`unreachable` when no pair exists. An empty initial space returns `empty`.
If enumeration is incomplete and there is no pair, the verdict is
`unknown`. A pair found before exhaustion remains valid with
`complete: false`, with no shortest-path guarantee. The comparison uses
the selected final snapshots and can pair histories of different lengths.

Set `"comparison":"history"` to compare the initial observations and the
observations after each public operation. By default every operation
produces an observation, including an explicit no-op. Command names and
arguments can differ between the matching public steps.

Supply `"silent":["retry"]` to let either side take those operations
without sampling an observation. Internal operations still count toward
the shortest total command count and appear in the returned traces.
Their state changes can affect later observations. Both final states
must satisfy `at`. The budget includes configurations, initial candidates
and pairs, action bindings, silent moves, and public successor pairs. For a transient notification example, use `notice-history.ideal`
with `StrictDone` or `OptionalDone`.

For a conditionally public operation, use an `emit` rule:

```json
{
  "at": "Finished",
  "comparison": "history",
  "observe": ["phase", "publicExhaustion"],
  "distinguish": ["attempts"],
  "emit": [{
    "action": "retry",
    "event": "exhausted",
    "when": "some after(exhaustion) and no before(exhaustion)"
  }]
}
```

Try it with `examples/model-patterns/subscriptions/retry-threshold.ideal`.
The optional `when` is a transition expression with the operation's
arguments in scope. If it is omitted, the rule always emits. If it is
false, the operation takes a silent step. A named event matches the same
name and the same `observe` fields from the resulting state. An operation
may have one rule and cannot also appear in `silent`. The query applies
these rules during reference exploration.

Both `observe` and `distinguish` accept state fields and model-level
`derive` names in snapshot and history comparisons. A derived field in
an emitted event is a sorted array of tuples. For example, a unary value
is `[["Done"]]`, a binary pair is `[["Pending","Exhausted"]]`, and
an empty result is `[]`. State fields keep their generated binding format.
`idealis-check generate <model> ir` saves the checked expressions in the optional
`derivations` field, so these queries work with the generated JSON too.

Each history trace has an `events` array. Entries have a one-based
`step` into that trace's `commands`, the event name, and the projected
`observation`. The array contains named emissions from `emit` rules.
Ordinary observations for actions without a rule are still used in pair
matching. The initial observation has no event. Two traces may list the
same public events at different steps.

The TypeScript example helper `replayEvents` checks these predictions
against an adapter's cumulative `observeEvents()` after each operation,
alongside the ordinary state comparison. The operation must settle the
observations that are due at that step before returning.

Snapshot sets are sorted by their JSON representation in witnesses,
`validate` responses and generated states. Sequences keep their element
order in the `$sequence` field. Validation accepts set elements in any
order.

A model may name an action argument `kind`. In command JSON and the
generated TypeScript `Command` type, that argument uses `"$kind"`.
For example, `choose(kind: Choice)` with argument `Second` becomes
`{"kind":"choose","$kind":"Second"}`. Schema, witness export, validation,
and the generated decoders use the same key.

## Dead behavior

```sh
idealis-check dead-code model.model
idealis-check dead-code model.model --max-states 200000
idealis-check dead-code examples/model-patterns/checkout/checkout.ideal --instances capture
```

The reference evaluator checks whether each action's guard holds for some
argument binding in a reachable state. JSON reports `coverage` and a status
for each action. A witness makes it `reachable`. A complete, nonempty
search with no witness makes it `unreachable`. A search that runs out of
budget or leaves the integer scope reports unseen actions as `unknown`.
A model without initial states reports `empty`. The budget counts
configuration candidates, initial candidates, and reached states.
Reference searches take fixed values from unconditional facts and initial
constraints, then check the full constraints on the remaining candidates.
`Declarations` checks multiplicity violations in updates separately.

`--instances ACTION` adds the selected action's argument bindings to
`instances`. Each entry has an `action`, an `args` object with values
in the bindings' JSON format, and a `status`. In the checkout example,
`capture` is reachable and its amount-zero binding is unreachable.
The option accepts at most 4,096 bindings for the selected action. Bindings
cover the declared finite domains, including inputs the guards intentionally
reject.

Exit status is 0 for complete coverage with no unreachable action or selected
instance, 1 if one is unreachable, 2 for invalid or unsupported input
(including IR with adapter omissions), and 3 for unknown or empty coverage.

## Audit reachability across a model

```sh
idealis-check coverage examples/model-patterns/crash-recovery/receipt-wire.ideal --max-states 20000
idealis-check coverage examples/model-patterns/subscriptions/partition-snapshot.ideal --instances
```

The report uses one reference search to evaluate every operation and the
conditional updates recorded from source. Each operation has a `status` and a `branches`
array with the same source locations and replayable traces as `idealis-check branches`.
For example, `deliver` is reachable in the receipt model, but the arm that
records conflicting decoded results is unreachable.

`coverage` describes operation exploration and `branchCoverage` describes
update groups. An IR without a branch map reports operation reachability and
sets `branchCoverage` to `unsupported`, with `branches: null`. For branch
diagnostics, use the source, the project manifest, or the IR with a branch
map. The report includes conditionals anywhere in assignment values, including
record fields and local bindings expanded into those values. The
`embedded_value_conditionals` exclusion still covers predicate evaluation,
such as a conditional inside an `if` condition or a `select` filter.

Pass `--instances` to include each operation's argument bindings in the same
search. The output adds `instances` to each operation and `instanceCoverage`
to the report. In the partition snapshot model `read` is reachable and
reading `Orders` at `Three` is unreachable. The catalogue has a total limit of
4,096 bindings, assigned in model order. An operation whose catalogue cannot
be included reports `instances: null` and an `instanceDiagnostic`. The
remaining catalogues are still checked and `instanceCoverage` is `partial`.

Exit 0 (`clear`) means a nonempty complete search reached every operation
and every requested argument binding and update arm without an unreachable expectation, and confirmed
every such expectation. Exit 1 identifies a definite unexpected absence
(`unreachable`) or a reached arm marked `expect unreachable`
(`expectation_mismatch`). Exit 3
(`inconclusive`) reports an empty or incomplete search, or an unsupported
branch check or a partial instance catalogue without a definite unreachable item. Exit 2 reports invalid
input. Where available, reached arms retain their witnesses after a budget is
exhausted. A scope exit or broken declaration leaves unobserved arms unknown.

## Find paths through conditional updates

```sh
idealis-check branches examples/model-patterns/subscriptions/partition-snapshot.ideal publish
```

Each source-level `if { … } then { … }` update group has a file, line,
and column under `source`, plus `then` and `else` results. A reachable arm
has a replayable trace ending with the selected operation. The search
uses the original conditions and the enclosing arms of each nested group.
Omitting `else` still yields a result for taking the implicit arm.

The report covers conditional update groups and conditionals in assignment
values in model source and linked projects. A value branch is marked
`"kind": "value"`. For a conditional written in a local update binding,
`binding` identifies the local name. An arm is reached when an enabled
update evaluates any of the binding's reported assignment uses. A binding
used only in predicates or with no assignment uses contributes no rows.
For example, run `idealis-check branches` on the `close` operation
in `examples/model-patterns/stream-terminal/split-terminal.ideal` to
inspect the three terminal boundaries. JSON IR contains flattened
assignments. The branch map supplies source locations and nesting.

With `complete: true`, each found trace is shortest and an absent arm is
`unreachable`, or `empty` when the model has no initial state. With incomplete
exploration, absent arms are `unknown`; any already found trace is kept.
Use `--max-states` and `--max-arguments` to set search budgets. The command
prints the JSON report; inspect the per-arm verdicts to gate coverage.

To document an arm that is deliberately unreachable, put
`expect unreachable "reason"` between `then` or `else` and the update
block. For example:

```text
if { accepted != decision }
  then expect unreachable "Compatible encodings decode to the same decision" {
    conflicts = conflicts + attempt
  }
```

The annotation makes a coverage expectation. The search still explores
the arm if its condition becomes true. An annotated arm reports its actual
`verdict`, the `reason`, and whether the `expectation` is `matched`,
`mismatch`, or `unknown`. Incomplete exploration or an empty initial
space leaves an unobserved expectation `unknown`. An explicit empty
block `{}` can be used to annotate the else arm that preserves state.

For a direct conditional assignment, put the same annotation before the
selected value.

```text
closed = if { fenceOffer in closedAt } then true
         else expect unreachable "The required checkpoint is the closed revision" false
```

The expectation is recorded in the branch map alongside the source
position. Conditionals nested under another expression or in a local
binding cannot carry these expectations.

### Carry branch diagnostics alongside the IR

```sh
model=examples/model-patterns/subscriptions/partition-snapshot.ideal
idealis-check generate "$model" ir > model.json
idealis-check generate "$model" branch-map > branches.json
idealis-check coverage model.json --branch-map branches.json
idealis-check branches model.json publish --branch-map branches.json
```

Generate both files from the same source revision. The branch map has its
own format version and binds its positions and selectors to the canonical
typed IR. A different IR or corrupted map produces exit 2. During replay,
the locations identify the original fragment even when the source files
are absent. Version 4 stores paths for inline conditionals inside assignment
operands. Version 3 stores paths for conditionals from expanded local
bindings, version 2 records direct conditional assignments, and version 1
describes update groups only. Earlier maps remain readable. Generation
uses the earliest of versions 2 through 4 that records all the paths.

## Find a state with no enabled operation

```sh
idealis-check deadlock examples/model-patterns/worker-locks/reversed.project.json --unless BothFinished
idealis-check deadlock examples/model-patterns/worker-locks/ordered.project.json --unless BothFinished
idealis-check deadlock examples/model-patterns/worker-barrier/reuse.ideal --unless EveryonePassed --ignore-stutter
```

`deadlock` searches reachable states for one where every operation's guards
reject every argument binding. `--unless NAME` excludes states satisfying
the formula of a named `reachable` declaration such as `BothFinished`.
An enabled operation counts even if it leaves the state unchanged.

With `--ignore-stutter`, enabled operations whose valid successor equals the
current state are omitted when deciding whether the state has an exit.
For the barrier this lets a wait probe run without masking a stalled round.
The query tests each state's immediate successors. Cycles between distinct
states require a temporal check.

The JSON includes a replayable trace when a deadlock is found. With complete
coverage, the trace is shortest across the configurations. The reversed lock
order reaches a circular wait after two acquisitions. With a shared lock
order, the only state without an enabled operation is orderly completion.

Exit status is 0 when the complete search rules out a deadlock, 1 when it
finds a deadlock, 2 for invalid input, and 3 for empty or inconclusive
exploration. State or argument budget limits, integer scope exits, and
multiplicity violations carry diagnostics. An update that would violate its
declaration still has an enabled guard. If the search finds a deadlock
elsewhere, it still returns that witness with `complete: false`.
This also applies with `--ignore-stutter`: a scope exit or malformed
successor prevents a stall verdict at its source state.

## Check whether a data type has values

```sh
idealis-check refinements examples/model-patterns/payment-data/payment-refined.ideal --type Request
```

`domain` solves the type's `where` clauses with rusmt over mathematical
integers and nonnegative naturals. `scope` answers the same question with
the model's integer interval applied to the value's fields. For example,
`data Amount = Nat where { value >= 10 }` with `scope Int = 0..2`
returns `domain: sat` with a value and `scope: unsat`. A refinement
requiring a negative `Nat` returns `domain: unsat`.

The value has the same JSON shape as the generated decoders expect, so
it can seed a test. This command checks type domains without evaluating
`init`, guards, or transitions. Specify `--type NAME` to select one
data type. By default it checks declarations that have a `where` clause.
Source files and projects retain declaration locations. JSON IR inputs
include the type name.

The exit code is 1 if any selected type is empty, 3 if a check is
undetermined or no refinement is selected, and 0 if each selected type
has a value and all scoped queries completed. A scope without values
does not change that exit code. `--max-work N` sets the rusmt work
budget independently for each query.

## Lean snapshot oracle

```sh
idealis-check lean-oracle fixtures/rcheck/bank.model formal/rmodel/Bank.lean
```

The reference evaluator must finish a nonempty search within the model
scope and the `--max-states` budget. idealis-check runs the linked Lean file with
`lean --run`, passing a JSON array of `{ "config": ..., "state": ... }`
observations on stdin. The values use the same encoding as the generated
Rust and TypeScript bindings. The file's `main` returns a JSON array with
one boolean per observation in input order. [The bank oracle](../../formal/rmodel/Bank.lean)
defines an independent Lean `Prop` for balances and evaluates it with `decide`.

A false result returns its configuration and state and exits 1. Empty
or incomplete exploration exits 3 before invoking Lean. A model declaration
violation, Lean failure, or invalid response exits 2. A successful result
covers the finite set of observations. Lean theorems have the scope of their
own statements. Run `just idealis-check-lean-oracle` with Lean on PATH to exercise
the normal bank, the weakened model, and invalid oracle responses.

## Implementations

`idealis-check generate <model> rust|typescript|schema|states` writes bindings for
an implementation: a Rust trait with one method per action, TypeScript types
with a JSON Lines driver, and JSON Schema of the configuration, the state and
the commands. `idealis_check::conform` replays the implementation along the
reference semantics and reports the shortest command sequence where it
differs.

`idealis-check generate <model> typescript-types` emits TypeScript declarations
for the model's values, configuration, state and commands. Save the output as
a `.d.ts` file and consume it with `import type` in the adapter. Validation of
runtime values is available through `validate` and replay. For projects with
an export list, `--public` limits the generated command union to that list.

The [runtime model patterns](../../examples/model-patterns/README.md) show how to connect a browser, a Node.js callback batch, or an object store to model predictions through an asynchronous observation adapter.

## Split sources and boundary properties

A `*.project.json` file lists ordered `fragments`, for example
`{"fragments":["domain.ideal","withdrawal.ideal"]}`. The legacy `.rmodel.json` suffix also works. Both `.ideal` and
`.model` fragments are accepted. Paths are relative
to the manifest. The fragments are parsed together with one model name and
one namespace. This is an input format for idealis-check, with the usual Idealis
syntax in each fragment. See the [withdrawal](../../fixtures/rcheck/linked/withdrawal/project.rmodel.json)
and [Raft election](../../fixtures/rcheck/linked/raft/project.rmodel.json)
projects. The withdrawal uses a record for money and an enum for outcomes.
A request can be accepted and later complete with `InsufficientFunds`.

A fragment can also instantiate a component file. The same path can appear
under distinct namespaces.

```json
{
  "path": "storage.ideal",
  "namespace": "east",
  "ports": { "receipt": { "state": "eastReceipt", "access": "write" } }
}
```

Use `@name` for a component declaration or reference. An unbound
`@head` becomes `east_head`. A bound `@receipt` resolves to the
project-level state variable `eastReceipt`. Each shared variable can
have one write port, and a write port also permits reads. Actions can
assign their instance's own variables and shared variables granted
through write ports. Project actions can assign unclaimed project
variables. Namespaces prefix declarations and generated commands.
Project formulas can refer to the flattened names. Plain fragments
remain in the shared project scope.

When loading the manifest, idealis-check checks state reads in each component
against its local variables and declared port bindings. This includes
reads through derived expressions, guards, updates, and properties.
Components may use the project's types and static relations. The project
assembles the initial-state contract and may inspect linked state.
Generated IR contains the linked model. Input restrictions on the
original component files are checked when reading the manifest.

Use a top-level `exports` array to select the operations in public
bindings, using their linked names. For example, the two-stores project
uses `"exports": ["east_commit", "west_commit"]`.

```sh
idealis-check generate examples/model-patterns/offline-merge/two-stores/model.project.json go --public
```

`--public` generates Rust, TypeScript, Go, MoonBit, or JSON Schema with
only the exported operations in command types, decoders, and interfaces.
An empty list produces a surface with no commands. `Config` and `State`
still describe the model's configuration and observation snapshots.
Rust public bindings include the trait and codecs. Use the ordinary
generation target for a driver that checks the complete model.

Checks, witnesses, validation, IR, and PBT generation use all operations,
including internal delivery. Ordinary binding generation also uses all
operations for replay. Exports must name distinct operations that exist
after linking.

### Type-check a component on its own

A `.component.json` descriptor identifies an interface source, shared
type fragments, and the component body. It declares the access to each
interface state variable.

```json
{
  "interface": "receipt-interface.ideal",
  "types": ["request-types.ideal"],
  "body": "delivery.ideal",
  "ports": { "receipt": "read" }
}
```

The interface uses ordinary Idealis syntax for its model name, finite
scope, and port variables. The type fragments declare the shared types.
The body uses `@receipt` and local names such as `@received`.

```sh
idealis-check component examples/model-patterns/offline-merge/two-stores/delivery.component.json
```

The command reports the checked port shapes, read/write access, and
operation names. Reachability and other properties are checked in the
linked project. A project instance supplies `"component": "delivery.component.json"`,
a `namespace`, and a `ports` map from interface names to project state
variables. idealis-check checks the component before linking, requires the same
shared type fragments in the project, and checks exact port shapes and
multiplicities. The combined model uses the project's scope.

See the [two stores example](../../examples/model-patterns/offline-merge/two-stores/README.md)
for two instances, a shared port, and a SQLite replay.

Source diagnostics report the original file and a one-based line and column.
Syntax diagnostics identify the token. Type diagnostics report the failing
operand for numeric value comparisons, or the enclosing declaration when no
operand position is available. A missing numeric value of multiplicity
`lone` includes a hint to check its presence with `some`. Model-wide checks
report the model input path.

```sh
model=fixtures/rcheck/linked/withdrawal/project.rmodel.json
idealis-check "$model" --checker reference
idealis-check generate "$model" typescript > bindings.ts
idealis-check generate "$model" pbt-cases > cases.json
idealis-check generate "$model" pbt-typescript > boundary.ts
just idealis-check-boundaries
```

`pbt-cases` emits versioned JSON. The reference evaluator first checks that
it can completely explore the finite model. For each configuration, idealis-check
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
[withdrawal test](../../tests/idealis-check/withdrawal.pbt.test.ts) for an adapter
using the generated bindings and a replay with a stored seed and path.

### Test data type constraints at the input boundary

```sh
model=examples/model-patterns/payment-data/payment-refined.ideal
idealis-check generate "$model" typescript > bindings.ts
idealis-check generate "$model" pbt-data-cases > data-cases.json
idealis-check generate "$model" pbt-data-typescript > data-boundary.ts
```

Call `checkDataBoundaries({ Amount: decodeAmount, Request: decodeRequest })`
from a test using the generated TypeScript bindings. Each case names a data
type, its JSON input, and whether the decoder should accept it. The generator
uses valid values from the finite model to fill the surrounding fields. It
probes below, at, and above the comparison bound. A literal bound can yield
cases outside the search scope, such as 999, 1000 and 1001 in the payment
example. It emits comparisons for which it finds both an accepted and a
rejected input.

### Check a component's behavior in a project

An optional `contract` in the `.component.json` descriptor names checks
declared in the component body. `assumptions` and `guarantees` name local
`invariant` or `transition` declarations. `exercises` names `reachable`
checks so a project can require a concrete exercise of its inputs and behavior.
Write names without the component body's `@` or the instance's namespace.
`idealis-check component` validates the names and kinds with the port types.

```sh
idealis-check contracts examples/model-patterns/offline-merge/two-stores/model.project.json
idealis-check contracts examples/model-patterns/offline-merge/two-stores/delivery-harness.project.json
```

Each instance reports the verdict and source location of every declared
check. Failed safety checks and reachable exercises include a replayable
path. The search explores the project's transitions and evaluates assumptions
and guarantees as properties of that graph. A consumer's contract
passes when its assumptions and guarantees hold and all listed exercises
are reachable. This is a finite check of the supplied composition or
harness. A read-only port can be driven by the harness as project state.

Exit 0 means every declared instance contract matched. Exit 1 identifies
a violated assumption or guarantee, or an unreachable exercise. Exit 3
indicates an inconclusive result, such as no declared checks, an empty
initial space or a spent search budget. Exit 2 reports an invalid project
or ill-typed contract.

### Generated Go interface and fuzz test

Generate both files in the same Go package. For the withdrawal example the
package is `withdrawal`.

```sh
idealis-check generate "$model" go > bindings.go
idealis-check generate "$model" pbt-go > boundary_generated_test.go
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
`just idealis-check-go` to run the [withdrawal implementation and fault control](../../tests/idealis-check/go/withdrawal/withdrawal_test.go).

## Validate before running a command

Use a checked model to create a Rust `Validator`. Start a session with
the supplied configuration and initial state. The session checks the wire
types, finite scope, facts, initial predicates and invariants.

The validator decodes just the supplied values according to their declared
sorts. A configuration, initial state, or command can be validated even
when its full domain exceeds a checker's enumeration limit. The
[saved queue replay](../../tests/model-patterns/restored-queue.test.ts)
exercises this with a witness and a SQLite implementation.

```rust
let validator = idealis_check::conform::Validator::new(&model)?;
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

To compare a computed observation, add `"derive": ["screen"]` to the
request. The response includes `derived.before` and `derived.after` maps
for the snapshots immediately before and after `command`. Each derived
value is a sorted array of tuples, such as `[["SignIn"]]`. Supply distinct
model-level derivation names. The `state` response and `initial` input
retain the generated state schema. Omitting `derive` or passing an empty
list returns just `state`.

Omit `command` to validate and return the current snapshot after the
optional prefix. With no prefix this validates the initial snapshot alone.
If `derive` is requested, `derived` is a single map for that snapshot.

```sh
idealis-check validate fixtures/rcheck/nat-account.model - <<'JSON'
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

## Save and replay implementation observations

Write a JSON object containing `config`, `initial`, the planned
`commands` array, and the collected `observations` array. The first
observation is the implementation's initial state. The remaining
observations are full `State` snapshots in the bindings' JSON form
after each command.
For a startup-only recording, use an empty `commands` array and one
initial observation. A difference in this frame is reported at step 0.

```sh
idealis-check record-itf bank.model observations.json > saved.itf.json
idealis-check replay-itf /path/to/bank.model saved.itf.json
```

The first argument to `record-itf` labels the trace. Choose the model
explicitly when replaying, including when the recording was made on
another machine. Both commands accept `-` for input on stdin.
`replay-itf` uses the same model checks as `validate`. Its output is
`matched` with the replayed step count, or `mismatch` with the command,
step, expected snapshot and recorded snapshot at the first difference.
The exit codes are 0 for a match, 1 for a mismatch, and 2 for invalid
input, including a missing observation when the recorded prefix matches.
A prefix ending in a difference can retain the rest of the planned commands.

The finite replay profile stores `config`, `command`, and `observation` in
each frame, with signed 64-bit integers in `#bigint` and absent optional
values in an empty `#tup`. It preserves the order of `$sequence`. Adapters
can project environment-specific observations into the bindings' snapshots
before writing them. [ITF format](https://apalache-mc.org/docs/adr/015adr-trace.html)

## Structured values and progress checks

```text
sequence Journal = Entry, max = 2
variant Outcome = Pending | Paid(Amount) | Rejected(Reason)
```

Use `Journal[A, B]`, `append(log, entry)`, `tail(log)`, `len(log)` and `at(log, index)`.
`tail(log)` removes the first entry and returns an empty sequence on empty input.
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

Run `just idealis-check-structured` for model checks, `just idealis-check-adapters` for Pkl,
and `just idealis-model-structured-tlc .artifacts/structured-replay` for TLC.

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

Use `transition Name on deliver { ... }` to constrain a single operation.
The formula can use that operation's parameters in either snapshot. In
the retry example, the actual request ID selects the effect whose value
must stay the same on redelivery. [Retry model](../../examples/model-patterns/retry/delivery.ideal)

`just idealis-check-transition` runs the regression tests.
`just idealis-model-transition-tlc .artifacts/transition-replay` checks the
lowered models with TLC. The reference and explicit checkers support
these properties. The SAT backend also checks transitions in its Alloy
fragment up to the model’s `steps` bound. For `on`, it checks each binding
of the named operation that can produce the step, even if another binding
or operation produces the same pair of snapshots.
