use rtlc::ir::{
    Binding, BodyKind, Branch, Definition, Domain, Expression, FoldOrder, Predicate, Scalar, Step,
    TemporalFormula, TemporalProperty, Update,
};
use rtlc::{SafetyBounds, SafetyProgram, Value};

const DEPTH: usize = 100_000;
const STACK_BYTES: usize = 8 * 1024 * 1024;
const EXPRESSION_REFUSAL: &str = "Safety expression nesting exceeds 48";
const TEMPORAL_REFUSAL: &str = "Temporal formula nesting exceeds 40";

fn truth() -> Expression {
    Expression::Literal {
        value: Scalar::Bool(true),
    }
}

fn atom() -> TemporalFormula {
    TemporalFormula::Atom {
        expression: truth(),
    }
}

fn definition(
    id: usize,
    parameter_count: usize,
    capture_count: usize,
    body: Expression,
) -> Definition {
    Definition {
        recursive: false,
        id,
        parameter_count,
        capture_count,
        body_kind: BodyKind::Expression,
        local_scope: 0,
        body: body.into(),
    }
}

fn program(body: Expression) -> SafetyProgram {
    SafetyProgram {
        format: "modelt.safety-program".into(),
        version: 2,
        name: "DeepDrop".into(),
        variables: vec!["x".into()],
        initial: vec![],
        next: vec![],
        invariants: vec![],
        assumptions: vec![],
        check_deadlock: false,
        symmetry: None,
        initial_properties: vec![],
        action_properties: vec![],
        properties: vec![],
        fairness: vec![],
        constraints: vec![],
        action_constraints: vec![],
        definitions: Some(vec![
            definition(0, 0, 0, body),
            definition(1, 1, 1, truth()),
        ]),
    }
}

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 8,
        max_states: 100,
        max_work: 10000,
        max_temporal_nodes: Some(10000),
    }
}

fn expression_chain(wrap: impl Fn(Expression) -> Expression) -> Expression {
    (0..DEPTH).fold(truth(), |inner, _| wrap(inner))
}

fn temporal_chain(wrap: impl Fn(TemporalFormula) -> TemporalFormula) -> TemporalFormula {
    (0..DEPTH).fold(atom(), |inner, _| wrap(inner))
}

fn frame_chain() -> Vec<Branch> {
    let leaf = vec![Branch {
        name: "Leaf".into(),
        steps: vec![Step::Guard {
            expression: truth(),
        }],
    }];
    (0..DEPTH).fold(leaf, |branches, _| {
        vec![Branch {
            name: "Nested".into(),
            steps: vec![Step::Frame {
                arguments: vec![],
                captures: vec![],
                parameter_count: 0,
                capture_count: 0,
                local_scope: 0,
                branches,
            }],
        }]
    })
}

fn enabled(steps: Vec<Step>) -> Expression {
    Expression::Enabled {
        branches: vec![Branch {
            name: "Step".into(),
            steps,
        }],
        subscript: None,
    }
}

fn binding(domain: Domain) -> Binding {
    Binding {
        tuple_size: None,
        slot: 0,
        domain,
    }
}

fn through_domain_value(inner: Expression) -> Domain {
    Domain::Value {
        value: Box::new(inner),
    }
}

fn with_temporal(formula: TemporalFormula) -> SafetyProgram {
    let mut input = program(truth());
    input.properties = vec![TemporalProperty {
        name: "Deep".into(),
        formula,
    }];
    input
}

fn on_eight_mebibytes(work: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(STACK_BYTES)
        .spawn(work)
        .unwrap()
        .join()
        .expect("the deep program is checked and dropped without panicking");
}

fn assert_refused_then_dropped(
    build: impl FnOnce() -> SafetyProgram + Send + 'static,
    message: &'static str,
) {
    on_eight_mebibytes(move || {
        let input = build();
        let refusal = input
            .check(bounds())
            .map(|result| result.status)
            .map_err(|error| (error.code, error.message));
        assert_eq!(refusal, Err(("protocol", message.to_string())));
        drop(input);
    });
}

fn check_then_drop(build: impl FnOnce() -> SafetyProgram + Send + 'static) {
    on_eight_mebibytes(move || {
        let input = build();
        let _checked = input.check(bounds());
        drop(input);
    });
}

macro_rules! deep_expression {
    ($name:ident, |$inner:ident| $wrap:expr) => {
        #[test]
        fn $name() {
            assert_refused_then_dropped(
                || program(expression_chain(|$inner| $wrap)),
                EXPRESSION_REFUSAL,
            );
        }
    };
}

macro_rules! deep_temporal {
    ($name:ident, |$inner:ident| $wrap:expr) => {
        #[test]
        fn $name() {
            assert_refused_then_dropped(
                || with_temporal(temporal_chain(|$inner| $wrap)),
                TEMPORAL_REFUSAL,
            );
        }
    };
}

macro_rules! deep_value {
    ($name:ident, |$inner:ident| $wrap:expr) => {
        #[test]
        fn $name() {
            check_then_drop(|| {
                let value = (0..DEPTH).fold(Value::Bool(true), |$inner, _| $wrap);
                program(Expression::Data { value })
            });
        }
    };
}

deep_expression!(deep_drop_expression_frame_body, |inner| Expression::Frame {
    arguments: vec![],
    captures: vec![],
    parameter_count: 0,
    capture_count: 0,
    local_scope: 0,
    body: Box::new(inner),
});

deep_expression!(deep_drop_expression_frame_arguments, |inner| {
    Expression::Frame {
        arguments: vec![inner],
        captures: vec![],
        parameter_count: 1,
        capture_count: 0,
        local_scope: 0,
        body: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_frame_captures, |inner| {
    Expression::Frame {
        arguments: vec![],
        captures: vec![inner],
        parameter_count: 0,
        capture_count: 1,
        local_scope: 0,
        body: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_call_arguments, |inner| {
    Expression::Call {
        definition: 1,
        arguments: vec![inner],
        captures: vec![truth()],
    }
});

deep_expression!(deep_drop_expression_call_captures, |inner| {
    Expression::Call {
        definition: 1,
        arguments: vec![truth()],
        captures: vec![inner],
    }
});

deep_expression!(deep_drop_expression_unary_argument, |inner| {
    Expression::Prime {
        argument: Box::new(inner),
    }
});

deep_expression!(deep_drop_expression_binary_left, |inner| Expression::Add {
    left: Box::new(inner),
    right: Box::new(truth()),
});

deep_expression!(deep_drop_expression_binary_right, |inner| Expression::Add {
    left: Box::new(truth()),
    right: Box::new(inner),
});

deep_expression!(deep_drop_expression_if_condition, |inner| Expression::If {
    condition: Box::new(inner),
    consequent: Box::new(truth()),
    alternative: Box::new(truth()),
});

deep_expression!(deep_drop_expression_if_alternative, |inner| {
    Expression::If {
        condition: Box::new(truth()),
        consequent: Box::new(truth()),
        alternative: Box::new(inner),
    }
});

deep_expression!(deep_drop_expression_subseq_sequence, |inner| {
    Expression::Subseq {
        sequence: Box::new(inner),
        from: Box::new(truth()),
        to: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_subseq_to, |inner| Expression::Subseq {
    sequence: Box::new(truth()),
    from: Box::new(truth()),
    to: Box::new(inner),
});

deep_expression!(deep_drop_expression_set_values, |inner| Expression::Set {
    values: vec![inner],
});

deep_expression!(deep_drop_expression_tuple_values, |inner| {
    Expression::Tuple {
        values: vec![inner],
    }
});

deep_expression!(deep_drop_expression_record_fields, |inner| {
    Expression::Record {
        fields: vec![("field".into(), inner)],
    }
});

deep_expression!(deep_drop_expression_record_set_fields, |inner| {
    Expression::RecordSet {
        fields: vec![("field".into(), inner)],
    }
});

deep_expression!(deep_drop_expression_at_target, |inner| Expression::At {
    target: Box::new(inner),
    key: Box::new(truth()),
});

deep_expression!(deep_drop_expression_at_key, |inner| Expression::At {
    target: Box::new(truth()),
    key: Box::new(inner),
});

deep_expression!(deep_drop_expression_update_target, |inner| {
    Expression::Update {
        target: Box::new(inner),
        updates: vec![],
    }
});

deep_expression!(deep_drop_expression_update_path, |inner| {
    Expression::Update {
        target: Box::new(truth()),
        updates: vec![Update {
            path: vec![inner],
            value: truth(),
        }],
    }
});

deep_expression!(deep_drop_expression_update_value, |inner| {
    Expression::Update {
        target: Box::new(truth()),
        updates: vec![Update {
            path: vec![],
            value: inner,
        }],
    }
});

deep_expression!(deep_drop_expression_quantifier_body, |inner| {
    Expression::Forall {
        bounds: vec![],
        body: Box::new(inner),
    }
});

deep_expression!(deep_drop_expression_quantifier_domain_value, |inner| {
    Expression::Exists {
        bounds: vec![binding(through_domain_value(inner))],
        body: Box::new(truth()),
    }
});

deep_expression!(
    deep_drop_expression_quantifier_domain_range_lower,
    |inner| {
        Expression::Function {
            bounds: vec![binding(Domain::Range {
                lower: Box::new(inner),
                upper: Box::new(truth()),
            })],
            body: Box::new(truth()),
        }
    }
);

deep_expression!(
    deep_drop_expression_quantifier_domain_range_upper,
    |inner| {
        Expression::Filter {
            bounds: vec![binding(Domain::Range {
                lower: Box::new(truth()),
                upper: Box::new(inner),
            })],
            body: Box::new(truth()),
        }
    }
);

deep_expression!(deep_drop_expression_quantifier_domain_set_values, |inner| {
    Expression::MapSet {
        bounds: vec![binding(Domain::Set {
            values: vec![inner],
        })],
        body: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_choose_binding_domain, |inner| {
    Expression::Choose {
        binding: binding(through_domain_value(inner)),
        body: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_choose_body, |inner| {
    Expression::Choose {
        binding: binding(Domain::Set { values: vec![] }),
        body: Box::new(inner),
    }
});

deep_expression!(deep_drop_expression_fold_initial, |inner| {
    Expression::Fold {
        accumulator: 0,
        initial: Box::new(inner),
        collection: Box::new(truth()),
        order: FoldOrder::SetMembers,
        body: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_fold_collection, |inner| {
    Expression::Fold {
        accumulator: 0,
        initial: Box::new(truth()),
        collection: Box::new(inner),
        order: FoldOrder::SetMembers,
        body: Box::new(truth()),
    }
});

deep_expression!(deep_drop_expression_fold_body, |inner| Expression::Fold {
    accumulator: 0,
    initial: Box::new(truth()),
    collection: Box::new(truth()),
    order: FoldOrder::SequenceLeft,
    body: Box::new(inner),
});

deep_expression!(deep_drop_expression_in_value, |inner| Expression::In {
    value: Box::new(inner),
    domain: Domain::Set { values: vec![] },
});

deep_expression!(deep_drop_expression_in_domain_value, |inner| {
    Expression::In {
        value: Box::new(truth()),
        domain: through_domain_value(inner),
    }
});

deep_expression!(deep_drop_expression_enabled_subscript, |inner| {
    Expression::Enabled {
        branches: vec![],
        subscript: Some(Box::new(inner)),
    }
});

deep_expression!(deep_drop_expression_enabled_guard, |inner| enabled(vec![
    Step::Guard { expression: inner }
]));

deep_expression!(deep_drop_expression_enabled_assign, |inner| enabled(vec![
    Step::Assign {
        slot: 0,
        value: inner,
    }
]));

deep_expression!(deep_drop_expression_enabled_bind_domain, |inner| enabled(
    vec![Step::Bind {
        slot: 0,
        domain: through_domain_value(inner),
        tuple_size: None,
    }]
));

deep_expression!(deep_drop_expression_enabled_choose_domain, |inner| enabled(
    vec![Step::Choose {
        slot: 0,
        domain: through_domain_value(inner),
    }]
));

deep_expression!(deep_drop_expression_enabled_frame_arguments, |inner| {
    enabled(vec![Step::Frame {
        arguments: vec![inner],
        captures: vec![],
        parameter_count: 1,
        capture_count: 0,
        local_scope: 0,
        branches: vec![],
    }])
});

deep_expression!(
    deep_drop_expression_enabled_frame_captures,
    |inner| enabled(vec![Step::Frame {
        arguments: vec![],
        captures: vec![inner],
        parameter_count: 0,
        capture_count: 1,
        local_scope: 0,
        branches: vec![],
    }])
);

#[test]
fn deep_drop_step_frame_branches_in_enabled() {
    assert_refused_then_dropped(
        || {
            program(Expression::Enabled {
                branches: frame_chain(),
                subscript: None,
            })
        },
        EXPRESSION_REFUSAL,
    );
}

#[test]
fn deep_drop_step_frame_branches_in_next() {
    assert_refused_then_dropped(
        || {
            let mut input = program(truth());
            input.next = frame_chain();
            input
        },
        EXPRESSION_REFUSAL,
    );
}

#[test]
fn deep_drop_step_frame_branches_in_initial() {
    assert_refused_then_dropped(
        || {
            let mut input = program(truth());
            input.initial = frame_chain();
            input
        },
        EXPRESSION_REFUSAL,
    );
}

#[test]
fn deep_drop_expression_in_invariant_predicate() {
    assert_refused_then_dropped(
        || {
            let mut input = program(truth());
            input.invariants = vec![Predicate {
                name: "Deep".into(),
                expression: expression_chain(|inner| Expression::Not {
                    argument: Box::new(inner),
                }),
            }];
            input
        },
        EXPRESSION_REFUSAL,
    );
}

deep_temporal!(deep_drop_temporal_frame_formula, |inner| {
    TemporalFormula::Frame {
        arguments: vec![],
        captures: vec![],
        parameter_count: 0,
        capture_count: 0,
        local_scope: 0,
        formula: Box::new(inner),
    }
});

deep_temporal!(deep_drop_temporal_unary_formula, |inner| {
    TemporalFormula::Not {
        formula: Box::new(inner),
    }
});

deep_temporal!(deep_drop_temporal_binary_left, |inner| {
    TemporalFormula::And {
        left: Box::new(inner),
        right: Box::new(atom()),
    }
});

deep_temporal!(deep_drop_temporal_binary_right, |inner| {
    TemporalFormula::Or {
        left: Box::new(atom()),
        right: Box::new(inner),
    }
});

#[test]
fn deep_drop_temporal_quantifier_formula() {
    assert_refused_then_dropped(
        || {
            with_temporal(
                (0..DEPTH)
                    .rev()
                    .fold(atom(), |inner, level| TemporalFormula::Forall {
                        bounds: vec![Binding {
                            tuple_size: None,
                            slot: level,
                            domain: Domain::Set { values: vec![] },
                        }],
                        formula: Box::new(inner),
                    }),
            )
        },
        "Invalid temporal binding count",
    );
}

deep_value!(deep_drop_value_list, |inner| Value::List(vec![inner]));

deep_value!(deep_drop_value_set, |inner| Value::Set(vec![inner]));

deep_value!(deep_drop_value_record, |inner| Value::Record(vec![(
    "field".into(),
    inner
)]));

deep_value!(deep_drop_value_map_key, |inner| Value::Map(vec![(
    inner,
    Value::Bool(true)
)]));

deep_value!(deep_drop_value_map_value, |inner| Value::Map(vec![(
    Value::Bool(true),
    inner
)]));
