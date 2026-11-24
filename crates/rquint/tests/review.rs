//! Regressions for the review of #102: each case is a small IR built here
//! and checked through rtlc.
use serde_json::{Value, json};

fn app(opcode: &str, args: Vec<Value>) -> Value {
    json!({"kind": "app", "opcode": opcode, "args": args})
}
fn name(name: &str) -> Value {
    json!({"kind": "name", "name": name})
}
fn int(value: i64) -> Value {
    json!({"kind": "int", "value": value})
}
fn def(qualifier: &str, name: &str, expr: Value) -> Value {
    json!({"kind": "def", "qualifier": qualifier, "name": name, "expr": expr})
}
fn lambda(params: &[&str], expr: Value) -> Value {
    let params: Vec<Value> = params.iter().map(|name| json!({"name": name})).collect();
    json!({"kind": "lambda", "params": params, "qualifier": "def", "expr": expr})
}
fn ir(declarations: Vec<Value>) -> String {
    let mut all = vec![json!({"kind": "var", "name": "x"})];
    all.extend(declarations);
    json!({"main": "M", "errors": [], "modules": [{"name": "M", "declarations": all}]}).to_string()
}

/// (status, states) of checking `invariant`.
fn check(ir: &str, invariant: &str) -> (String, usize) {
    let translation = rquint::translate(
        ir,
        &rquint::Options {
            invariants: vec![invariant.to_owned()],
            ..rquint::Options::default()
        },
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let result = idealis_core::compile_tla_with_options(
        &translation.source,
        &translation.config,
        idealis_core::CompileOptions {
            irVersion: idealis_core::IrVersion::V2,
        },
    )
    .unwrap_or_else(|error| panic!("{error:?}\n{}", translation.source))
    .check(idealis_core::SafetyBounds {
        max_depth: 100,
        max_states: 10_000,
        max_work: 1_000_000,
        max_temporal_nodes: None,
    })
    .unwrap_or_else(|error| panic!("{error:?}\n{}", translation.source));
    (result.status, result.states)
}
fn steady(extra: Vec<Value>) -> Vec<Value> {
    let mut declarations = vec![
        def("action", "init", app("assign", vec![name("x"), int(0)])),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
    ];
    declarations.extend(extra);
    declarations
}

/// Quint truncates division toward zero; the remainder has the dividend's
/// sign (checked with Quint 0.33.1's simulator).
#[test]
fn division_truncates_toward_zero() {
    let facts = app(
        "and",
        vec![
            app("eq", vec![app("idiv", vec![int(-7), int(2)]), int(-3)]),
            app("eq", vec![app("imod", vec![int(-7), int(2)]), int(-1)]),
            app("eq", vec![app("idiv", vec![int(7), int(-2)]), int(-3)]),
            app("eq", vec![app("imod", vec![int(7), int(-2)]), int(1)]),
            app("eq", vec![app("idiv", vec![int(7), int(2)]), int(3)]),
            app("eq", vec![app("imod", vec![int(7), int(2)]), int(1)]),
            // State-dependent operands take the same path.
            app(
                "eq",
                vec![
                    app("idiv", vec![app("isub", vec![name("x"), int(7)]), int(2)]),
                    int(-3),
                ],
            ),
        ],
    );
    let model = ir(steady(vec![def("val", "facts", facts)]));
    assert_eq!(check(&model, "facts"), ("complete".to_owned(), 1));
}

/// A call of a global operator orders the definitions like a name does.
#[test]
fn operator_calls_order_definitions() {
    let model = ir(steady(vec![
        def("def", "z", lambda(&["n"], name("n"))),
        def("val", "a", app("eq", vec![app("z", vec![int(1)]), int(1)])),
    ]));
    assert_eq!(check(&model, "a"), ("complete".to_owned(), 1));
    // rtlc resolves forward references; SANY and TLC do not.
    let source = rquint::translate(&model, &rquint::Options::default())
        .unwrap()
        .source;
    assert!(
        source.find("z(").unwrap() < source.find("a ==").unwrap(),
        "{source}"
    );
}

/// Generated names begin with q_; a user name with that prefix is refused
/// instead of colliding with a binder.
#[test]
fn reserved_prefix_is_refused() {
    let model = ir(steady(vec![def("val", "q_n_1", int(0))]));
    let error = rquint::translate(&model, &rquint::Options::default()).unwrap_err();
    assert!(error.0.contains("q_"), "{error}");
}

/// `init` called from `step` keeps its transition form there.
#[test]
fn init_used_as_a_step_branch() {
    let step = app(
        "actionAny",
        vec![
            name("init"),
            app(
                "actionAll",
                vec![
                    app("ilt", vec![name("x"), int(2)]),
                    app(
                        "assign",
                        vec![name("x"), app("iadd", vec![name("x"), int(1)])],
                    ),
                ],
            ),
        ],
    );
    let model = ir(vec![
        def("action", "init", app("assign", vec![name("x"), int(0)])),
        def("action", "step", step),
        def("val", "bounded", app("ilte", vec![name("x"), int(2)])),
    ]);
    assert_eq!(check(&model, "bounded"), ("complete".to_owned(), 3));
}

/// An action with parameters called from `init` constrains the first state.
#[test]
fn init_calls_a_parameterized_action() {
    let model = ir(vec![
        def(
            "action",
            "setX",
            lambda(&["n"], app("assign", vec![name("x"), name("n")])),
        ),
        def("action", "init", app("setX", vec![int(3)])),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
        def("val", "three", app("eq", vec![name("x"), int(3)])),
    ]);
    assert_eq!(check(&model, "three"), ("complete".to_owned(), 1));
}

/// Namespaced definitions of a flattened import are kept; only Quint's own
/// `q::` aliases are dropped.
#[test]
fn qualified_definitions_are_kept() {
    let model = ir(vec![
        def("pure", "Helpers::limit", int(3)),
        def(
            "action",
            "init",
            app("assign", vec![name("x"), name("Helpers::limit")]),
        ),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
        def("val", "atLimit", app("eq", vec![name("x"), int(3)])),
        def("action", "q::init", name("init")),
    ]);
    assert_eq!(check(&model, "atLimit"), ("complete".to_owned(), 1));
}

// Review of #103: the Alloy lowering.

fn lower(model: &str, invariant: &str, bounds: rquint::Bounds) -> Result<String, rquint::Error> {
    rquint::to_alloy(
        model,
        &rquint::Options {
            invariants: vec![invariant.to_owned()],
            ..rquint::Options::default()
        },
        bounds,
    )
}
fn sat(source: &str, label: &str) -> ralloy::Status {
    let model = ralloy::Model::parse(source).unwrap_or_else(|error| panic!("{error}\n{source}"));
    let index = model
        .commands()
        .iter()
        .position(|command| command.label == label)
        .unwrap();
    model.analyze(index, 50_000_000).unwrap().status
}
fn typed_ir(declarations: Vec<Value>) -> String {
    let mut all = vec![json!({"kind": "var", "name": "x", "typeAnnotation": {"kind": "int"}})];
    all.extend(declarations);
    json!({"main": "M", "errors": [], "modules": [{"name": "M", "declarations": all}]}).to_string()
}

#[test]
fn constant_false_is_unsatisfiable() {
    let model = typed_ir(steady(vec![def(
        "val",
        "never",
        json!({"kind": "bool", "value": false}),
    )]));
    let source = lower(&model, "never", rquint::Bounds::default()).unwrap();
    assert_eq!(sat(&source, "c_never"), ralloy::Status::Counterexample);
}

#[test]
fn the_smallest_integer_is_a_signed_literal() {
    let model = typed_ir(vec![
        def("action", "init", app("assign", vec![name("x"), int(-16)])),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
        def("val", "least", app("eq", vec![name("x"), int(-16)])),
    ]);
    let source = lower(&model, "least", rquint::Bounds::default()).unwrap();
    assert_eq!(sat(&source, "c_least"), ralloy::Status::NoCounterexample);
    // Alloy rejects the literal 16 at five bits; ralloy happens to wrap it.
    assert!(
        !source.contains("negate[16]") && source.contains("-16"),
        "{source}"
    );
}

#[test]
fn unsupported_widths_and_names_are_refused() {
    let model = typed_ir(steady(vec![def(
        "val",
        "overflow",
        json!({"kind": "bool", "value": true}),
    )]));
    let error = lower(&model, "overflow", rquint::Bounds::default()).unwrap_err();
    assert!(error.0.contains("overflow"), "{error}");
    let model = typed_ir(steady(vec![def(
        "val",
        "ok",
        json!({"kind": "bool", "value": true}),
    )]));
    for bitwidth in [0, 6] {
        let error = lower(&model, "ok", rquint::Bounds { bitwidth, steps: 8 }).unwrap_err();
        assert!(error.0.contains("bit width"), "{error}");
    }
}

#[test]
fn constant_folding_does_not_overflow_the_host() {
    let big = int(65_536);
    let product = app(
        "imul",
        vec![
            app("imul", vec![big.clone(), big.clone()]),
            app("imul", vec![big.clone(), big]),
        ],
    );
    let model = typed_ir(vec![
        def("action", "init", app("assign", vec![name("x"), product])),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
        def("val", "ok", json!({"kind": "bool", "value": true})),
    ]);
    let error = lower(&model, "ok", rquint::Bounds::default()).unwrap_err();
    assert!(error.0.contains("64-bit"), "{error}");
}

#[test]
fn non_integer_choices_are_refused() {
    let choice = json!({
        "kind": "let",
        "opdef": def("nondet", "b", app("oneOf", vec![app("Set", vec![json!({"kind": "bool", "value": true})])])),
        "expr": app("assign", vec![name("x"), int(0)]),
    });
    let model = typed_ir(vec![
        def("action", "init", app("assign", vec![name("x"), int(0)])),
        def("action", "step", choice),
        def("val", "ok", json!({"kind": "bool", "value": true})),
    ]);
    let error = lower(&model, "ok", rquint::Bounds::default()).unwrap_err();
    assert!(error.0.contains("oneOf"), "{error}");
}

#[test]
fn malformed_arity_is_an_error() {
    let model = typed_ir(vec![
        def("action", "init", app("assign", vec![])),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
        def("val", "ok", app("matchVariant", vec![])),
    ]);
    assert!(lower(&model, "ok", rquint::Bounds::default()).is_err());
    assert!(
        rquint::translate(
            &ir(vec![
                def("action", "init", app("assign", vec![name("x"), int(0)])),
                def("action", "step", app("assign", vec![name("x"), name("x")])),
                def("val", "ok", app("matchVariant", vec![])),
            ]),
            &rquint::Options {
                invariants: vec!["ok".into()],
                ..rquint::Options::default()
            }
        )
        .is_err()
    );
}

// Issue #105: name scoping in the TLA+ lowering.

fn let_val(name: &str, value: Value, body: Value) -> Value {
    json!({"kind": "let", "opdef": def("val", name, value), "expr": body})
}

/// A global action inlined from init sees the globals, not the caller's
/// local bindings.
#[test]
fn inlined_actions_do_not_see_caller_locals() {
    let model = ir(vec![
        def("val", "limit", int(3)),
        def(
            "action",
            "setX",
            lambda(
                &["n"],
                app(
                    "assign",
                    vec![name("x"), app("iadd", vec![name("n"), name("limit")])],
                ),
            ),
        ),
        def(
            "action",
            "init",
            let_val("limit", int(10), app("setX", vec![int(0)])),
        ),
        def("action", "step", app("assign", vec![name("x"), name("x")])),
        def("val", "three", app("eq", vec![name("x"), int(3)])),
    ]);
    assert_eq!(check(&model, "three"), ("complete".to_owned(), 1));
}

/// A local binding named like a global is not a dependency on it.
#[test]
fn local_names_are_not_dependencies() {
    let model = ir(steady(vec![
        def("val", "a", app("iadd", vec![name("b"), int(1)])),
        def("val", "b", let_val("a", int(1), name("a"))),
        def("val", "two", app("eq", vec![name("a"), int(2)])),
    ]));
    assert_eq!(check(&model, "two"), ("complete".to_owned(), 1));
}

/// Names that lower to the same TLA+ identifier are refused.
#[test]
fn lowered_name_collisions_are_refused() {
    let model = ir(steady(vec![
        def("pure", "Helpers::limit", int(3)),
        def("pure", "Helpers_limit", int(4)),
    ]));
    let error = rquint::translate(&model, &rquint::Options::default()).unwrap_err();
    assert!(error.0.contains("collide"), "{error}");
}
