# Idealis

The shared source and checked model types live in [idealis-syntax](../idealis-syntax),
and the reference evaluator lives in [idealis-semantics](../idealis-semantics).
This crate provides the existing Rust API and the backend and binding adapters.

One model of structure, state and behavior. Static relations (`rel`) are the
configuration space, `var` relations are the state, and actions change the
state with an implicit frame: a var an action does not assign keeps its
value. The same checked model is lowered independently to

- Alloy 6 text, solved by ralloy's SAT as a lasso of `steps` states
  (bounded), and
- TLA+ with a TLC configuration, searched by rtlc (complete). Static
  relations are chosen in `Init` from `SUBSET`, so the explicit search
  enumerates the configurations.

```
model Mutex
entity Proc "A process that can acquire locks."
entity Lock "A resource held by at most one process."
var holder: Lock -> lone Proc
init { no holder }
behavior acquire(p: Proc, l: Lock) {
  guard { no l.holder }
  update { holder = holder + l -> p }
}
invariant OneHolder { all l: Lock | lone l.holder }
reachable Handover { all l: Lock | some l.holder }
scope Proc = 2, Lock = 2, steps = 6
```

`behavior` accepts an optional `guard` block followed by an optional
`update` block. All guards must hold in the current state, and every
right-hand side in `update` is evaluated in that same state. The updates
take effect together. This is the same transition as
`action acquire(p: Proc, l: Lock) { no l.holder holder' = holder + l -> p }`.

An `update` block can group assignments under
`if { condition } then { assignments } else { assignments }`.
The `else` block is optional. Variables omitted from the selected branch
keep their previous values.

In a condition block, a comma can separate adjacent conditions. For example,
`invariant Consistent { no published, (some draft) implies ready }`.

In a `transition` property, `unchanged(expr)` expands to
`before(expr) = after(expr)`. See the [grammar cheatsheet](CHEATSHEET.md)
for runnable examples of state, transition, and temporal properties. The
[language and workflow review](review.md) records coverage, usability gaps,
and priorities from the model-pattern corpus.

`entity` gives a domain concept an identity and an optional description.
Use `entity Account "…"` for a one-line definition or
`entity Account description = """…"""` for multiple lines. A description map
such as `description = { ja: "口座", en: "Account" }` records translations.
Relations, parameters, and scopes refer to the same name. Rust and TypeScript
bindings copy descriptions to the generated type, and
`idealis-check generate <model> glossary-localized` exports all descriptions as JSON.
Existing `sig` declarations have the same finite-domain semantics.

```rust
let model = idealis_model::parse(source)?;
let alloy = ralloy::Model::parse(&idealis_model::to_alloy(&model))?;
let (tla, cfg) = idealis_model::to_tla(&model, "OneHolder")?;
```

The crate has no dependencies; engines are the caller's choice. Declaration
and check names are the identifiers a Pkl specification links its rules to.
See [SUPPORT.md](SUPPORT.md) and the [grammar decision log](grammar-decisions.md).
The [design and evaluation guide](design.md) ties grammar decisions to goals,
semantic checks, and evidence for adoption.
