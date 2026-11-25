# rprob

**Experimental · ProB-inspired.** rprob is a bitset exploration kernel for
the finite set-and-relation fragment of rmodel. Its input is checked rmodel
syntax. ProB input formats and constraint solving are outside its current
scope.

```sh
cargo run --release --locked --manifest-path crates/rprob/Cargo.toml -- fixtures/rprob/shipment.model
just rprob-test
just rprob-bench .artifacts/rprob-new
```

Sets use a bit per atom, and binary relations use a fixed bit matrix.
Candidate domains store small tables and generate larger choices on demand.
Functions and partial functions select legal values per row.
Lowering resolves slots and expands finite parameters and quantifiers
before search. Exploration uses a hash set of snapshots and a
breadth-first queue. Single-slot models can use an exact visited bitmap;
single- and two-slot states fit directly in their queues. Guards that only
read the configuration are evaluated once per configuration. The work budget counts each examined configuration,
initial candidate and reached state. The report includes the verdicts
and shortest witness depths, declared multiplicity violations, and
action reachability.

See [SUPPORT.md](SUPPORT.md) for the bounds and supported operators.
The [candidate enumeration experiment](../../evals/iterations/0125-rprob-candidates.md)
records the workload, measured cost, and remaining limits.
The [exploration profile and measurements](../../evals/iterations/0126-rprob-hot-paths.md) track the effects of each change.
The [stress benchmark](../../evals/iterations/0127-rprob-budget-and-dispatch.md) measures bulk budget accounting and specialized bit transitions.
