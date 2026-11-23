# rquint

Reads Quint's JSON IR (`quint compile --target json`) and writes standard
TLA+ with a TLC configuration, so rtlc (or TLC) checks Quint models without
Apalache or a JVM.

```rust
let ir = std::fs::read_to_string("Counter.json")?;
let translation = rquint::translate(
    &ir,
    &rquint::Options {
        invariants: vec!["safety".into()],
        ..rquint::Options::default()
    },
)?;
let program = idealis_core::compile_tla(&translation.source, &translation.config)?;
```

The output uses only `Integers`, `Sequences`, `FiniteSets` and `TLC`. Set
folds take elements in `CHOOSE` order, like Apalache's TLC definition of
`ApaFoldSet`; variants become `[tag |-> "Name", value |-> v]` records; Quint's
0-based list indices become TLA+'s 1-based ones. Unknown declarations,
operators and names are errors. See [SUPPORT.md](SUPPORT.md).
