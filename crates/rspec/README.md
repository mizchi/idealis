# rspec

A specification written in Pkl: a ubiquitous language of terms, each bound
to the model declarations and code symbols that denote it, and rules in
prose grounded by claims about rmodel checks.

- `rspec::links` decides what is static: unique ids, references that
  resolve, a closed vocabulary over the referenced models (every
  declaration has a term, every check a claim), and expectations that fit
  their checks.
- `rspec::render` writes the Japanese glossary and rules as Markdown.
- `rspec::documents_manifest` derives the `jev-lint docs` manifest that
  compares the glossary with every crate document (terminology) and each
  rule with its models (semantic). Those answers are advisory.

```sh
just spec-check        # links, claims through ralloy's SAT and rtlc, freshness
just spec-write        # regenerate JSON, Markdown and the manifest
just spec-drift        # jev-lint docs, appended to .jev-lint/judgments/spec
```

The specifications are `spec/idealis.pkl` (the vocabulary of idealis
itself, rendered to `docs/glossary.md`) and `fixtures/rmodel/spec.pkl`.
See [SUPPORT.md](SUPPORT.md).
