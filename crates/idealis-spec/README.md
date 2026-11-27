# idealis-spec

Idealis specifications connect domain vocabulary and natural-language rules
to formal properties and implementation symbols. The current document
adapter reads specifications written in Pkl. See the [language architecture](../../docs/language-architecture.md).

- `idealis_spec::links` decides what is static: unique ids, references that
  resolve, a closed vocabulary over the referenced models (every
  declaration has a term, every check a claim), and expectations that fit
  their checks.
- `idealis_spec::render` writes the Japanese glossary and rules as Markdown.
- `idealis_spec::documents_manifest` derives the `jev-lint docs` manifest that
  compares the glossary with every crate document (terminology) and each
  rule with its models (semantic). Those answers are advisory.

```sh
just spec-check        # links, model verdicts, freshness
just spec-write        # regenerate JSON, Markdown and the manifest
just spec-drift        # jev-lint docs, appended to .jev-lint/judgments/spec
```

The specifications are `spec/idealis.pkl` (the vocabulary of idealis
itself, rendered to `docs/glossary.md`), `fixtures/rmodel/spec.pkl`, and
`spec/rmodel-design.pkl`. The first two check claims through ralloy's SAT
and rtlc. The grammar design specification uses rmodel's reference evaluator.
See [SUPPORT.md](SUPPORT.md).

## Clause reviews

A rule with separately reviewable sentences can be declared as
`new s.ClauseRule`, where `s` is the import of `spec/Schema.pkl`.
Each clause has a local `id`, bilingual `text`, an optional model
`claim`, and an optional `reviewed` revision.

```sh
just clause-report
```

The example reports the shipment rule. It lists the clauses needing
attention first, followed by the link status, correspondence review, and
fresh reference-evaluator result for each clause. After reviewing the
displayed sentence and its named check, record the current `revision`
in the clause's `reviewed` field. The report recalculates the model
checks every time. Its exit code indicates whether it produced a report.

A correspondence revision covers the clause and shared rule text,
referenced terminology, the checked formula, its expected verdict, and
the model's declarations, facts and scope. The current version uses
rmodel's parsed AST representation. An AST format change can require a
new review. Body and guard edits affect the model verdicts. Semantic
review determines whether the natural-language sentence and formula mean
the same thing.

## Evidence for adoption

`just adoption-evidence --pin CANDIDATE` prints the evaluation revision
for the idealis adoption rule. Record it when scheduling a comparison
or measurement. Use an immutable candidate identifier, such as the source
revision or artifact digest of the actual implementation under evaluation.
The evaluation revision also includes the current requirement and the
linked models' full behavior.

Comparison and measurement inputs each have a `revision` field with the
whole pinned object and an `outcome` field. For example, a comparison can
return `equivalent`, `diverges`, `empty`, `incomplete`, or `missing`.
A measurement can return `improved`, `unmeasured`, `no-gain`, or
`regressed`. Preserve the revision from the evidence producer at delivery.

```sh
just adoption-evidence --assess CANDIDATE comparison.json goal.json
just evidence-adoption-test
```

The assessment recomputes the current clause report. Its JSON returns
`ready` only when the clause review, model verdicts, comparison and
measurement satisfy the current candidate. The command's exit status
indicates whether it produced a report.

For the single-model adoption rule, the reference evaluator and rprob can
produce the comparison input directly:

```sh
just adoption-evidence --plan CANDIDATE 100000 > plan.json
just checker-receipts run plan.json reference > reference.json
just checker-receipts run plan.json rprob > rprob.json
just checker-receipts collect plan.json reference.json rprob.json > comparison.json
```

Both workers verify the pinned model's source digest and use the pinned
budget. The collector requires complete results from both workers on
that plan, with equal verdicts, witness depths, scope, declarations, and
action enablement. It reports missing, empty, incomplete, and diverging
comparisons separately. Repeated identical results count once.
Conflicting repeats keep the comparison incomplete.

The `rprob` performance workload also produces the goal input:

```sh
just rprob-goal-pin > revision.json
just rprob-goal .artifacts/goal-new revision.json
just rprob-goal-replay .artifacts/goal-new
```

Use the candidate from `revision.json` for the comparison plan. Pass
`.artifacts/goal-new/goal-evidence.json` as the goal input to
`adoption-evidence --assess`. The benchmark compares the reference
evaluator and rprob on the same 3×4 workload, with the executable and
model pinned before measuring. Noisy or invalid measurements produce
unmeasured evidence. See the [measurement evaluation](../../evals/iterations/0124-measured-goal.md).
