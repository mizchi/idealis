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
