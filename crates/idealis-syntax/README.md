# idealis-syntax

Parse Idealis model and vocabulary sources. The crate owns the model types,
name resolution, type checks, and source locations. Start with
`parse_located_checked` for a source model or `validate_checked` for an
externally constructed model. A `CheckedModel` can be given to
[idealis-semantics](../idealis-semantics) for finite evaluation.

The [language architecture](../../docs/language-architecture.md) describes
the boundary and the [cheat sheet](../idealis-model/CHEATSHEET.md) shows source syntax.
