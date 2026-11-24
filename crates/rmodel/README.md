# rmodel

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
sig Proc
sig Lock
var holder: Lock -> lone Proc
init { no holder }
action acquire(p: Proc, l: Lock) {
  no l.holder
  holder' = holder + l -> p
}
invariant OneHolder { all l: Lock | lone l.holder }
reachable Handover { all l: Lock | some l.holder }
scope Proc = 2, Lock = 2, steps = 6
```

```rust
let model = rmodel::parse(source)?;
let alloy = ralloy::Model::parse(&rmodel::to_alloy(&model))?;
let (tla, cfg) = rmodel::to_tla(&model, "OneHolder")?;
```

The crate has no dependencies; engines are the caller's choice. Declaration
and check names are the identifiers a Pkl specification links its rules to.
See [SUPPORT.md](SUPPORT.md).
