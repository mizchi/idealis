# rprob

**Experimental · ProB-inspired.** rprob is a bitset exploration kernel for
finite sets and relations over atoms, scoped integers, records, tagged variants,
and bounded sequences. Its input is checked rmodel
syntax. ProB input formats and constraint solving are outside its current
scope.

```sh
cargo run --release --locked --manifest-path crates/rprob/Cargo.toml -- fixtures/rprob/shipment.model
just rprob-test
just rprob-bench .artifacts/rprob-new
```

Use `--safety` when a model includes both state or transition checks and
temporal questions. It checks the same finite state graph and prints
`unsupported (temporal)` for each omitted question. The default mode rejects
models with temporal questions. The same explicit choice is available as
`rprob::analyze_safety`, which returns the report and the omitted names.

```sh
cargo run --locked --manifest-path crates/rprob/Cargo.toml -- --safety examples/model-patterns/crash-recovery/transaction.model
```

The log compaction model exercises the scalar integer path with sequence
numbers and accumulated values. The `Scope` field reports a transition that
stores a value outside its declared range. Questions without a witness stay
unknown after such an exit.

```sh
cargo run --locked --manifest-path crates/rprob/Cargo.toml -- examples/model-patterns/lease/feed-compaction.model --max-states 20000
```

An oversized sequence domain can fit the bitset when its scalar state slots
start at known literals and every write selects a literal or copies such a
slot. The nested Node.js I/O batch uses two sequence values out of 341.
When the values cannot be reduced to literals, scalar slots can instead use
one ordinal per record or sequence. The checkpoint recovery model enumerates
2,728 possible initial values without allocating a table of them. Equality,
membership, absence, constructors and copies work in this representation.
An operation can take an indexed value as an argument. rprob compiles its
body once and tries bindings in domain order within the argument budget.
The queue refill model tries 116,623 histories and decodes the selected
argument in the witness. Both encodings support sequence transformations.
A temporary sequence can exceed its declared length during a comparison.
Storing a sequence beyond the bound reports `Scope`. An indexed record can
project a field directly from its ordinal. The restart budget example reads a
queue and retry count without building a table of checkpoints. A model can
derive a temporary set of indexed values with `elements`, use membership
and set algebra, and pass the result to `removeAll`. A functional relation can store indexed values when the packed rows fit
into one state word. Overwrite and key deletion operate on individual rows. [Restart budget](../../examples/model-patterns/crash-recovery/restart-budget.model) · [Queue refill](../../examples/model-patterns/crash-recovery/refill-queue.model) · [Retry rotation](../../examples/model-patterns/crash-recovery/rotate-queue.model) · [Batch journal](../../examples/model-patterns/crash-recovery/deduplicate-batch.model)

Benchmarks were tested with Hyperfine 2.0.0, including its `--metrics` flag. Use a fresh output directory for each run.

Sets use a bit per atom. Binary relations use an 8×8 bit matrix for models
with up to eight atoms. In larger models, each possible ordered pair is
assigned a shared bit, and products, joins and row replacements are compiled
against those pairs. The ring-copy example uses eleven atoms and four ordered
pairs. [Ring-copy model](../../examples/model-patterns/worker-deque/ring-copy.model)
Candidate domains store small tables and generate larger choices on demand.
Functions and partial functions select legal values per row.
Lowering resolves slots and expands finite parameters and quantifiers
before search. Exploration uses a hash set of snapshots and a
breadth-first queue. Single-slot models can use an exact visited bitmap;
single- and two-slot states fit directly in their queues. Guards that only
read the configuration are evaluated once per configuration. CLI checks,
`analyze_safety`, and witness searches count examined configurations, initial
candidates and reached states. They first bind values from unconditional
equalities or empty-set constraints in `fact` for configuration relations
and in `init` for state variables. The low-level
`Compiled::new` comparison path retains the full initial candidate charge
for models over plain atoms, matching `rmodel::reference::analyze`.
The report includes the verdicts
and shortest witness depths, declared multiplicity violations, and
action reachability.

See [SUPPORT.md](SUPPORT.md) for the bounds and supported operators.
The [candidate enumeration experiment](../../evals/iterations/0125-rprob-candidates.md)
records the workload, measured cost, and remaining limits.
The [exploration profile and measurements](../../evals/iterations/0126-rprob-hot-paths.md) track the effects of each change.
The [stress benchmark](../../evals/iterations/0127-rprob-budget-and-dispatch.md) measures bulk budget accounting and specialized bit transitions.
Use `just rprob-order-bench <output> <baseline>` for the [alternating-order dense workload](../../evals/iterations/0128-rprob-benchmark-order.md).
