use rtlc::ir::{
    BodyKind, Branch, Definition, Expression, SafetyTraceStep, SafetyViolation, Scalar, Step,
    TemporalFormula, TemporalProperty,
};
use rtlc::{SafetyProgram, SafetyResult, Value};

const SHALLOW_DEPTH: usize = 1_000;
const SHALLOW_STACK_BYTES: usize = 8 * 1024 * 1024;
const DEEP_DEPTH: usize = 200_000;
const DEEP_STACK_BYTES: usize = 512 * 1024;

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

fn definition(id: usize, body: Expression) -> Definition {
    Definition {
        recursive: false,
        id,
        parameter_count: 0,
        capture_count: 0,
        body_kind: BodyKind::Expression,
        local_scope: 0,
        body: body.into(),
    }
}

fn program(body: Expression) -> SafetyProgram {
    SafetyProgram {
        format: "modelt.safety-program".into(),
        version: 2,
        name: "DeepDropPaths".into(),
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
        definitions: Some(vec![definition(0, body), definition(1, truth())]),
    }
}

fn first_body(input: &mut SafetyProgram) -> &mut Expression {
    let rtlc::ir::DefinitionBody::Expression(body) = &mut input
        .definitions
        .as_mut()
        .expect("the program has definitions")[0]
        .body
    else {
        panic!("the definition contains an expression")
    };
    body
}

fn not_chain(depth: usize) -> Expression {
    (0..depth).fold(truth(), |inner, _| Expression::Not {
        argument: Box::new(inner),
    })
}

fn formula_chain(depth: usize) -> TemporalFormula {
    (0..depth).fold(atom(), |inner, _| TemporalFormula::Not {
        formula: Box::new(inner),
    })
}

fn frame_chain(depth: usize) -> Vec<Branch> {
    let leaf = vec![Branch {
        name: "Leaf".into(),
        steps: vec![Step::Guard {
            expression: truth(),
        }],
    }];
    (0..depth).fold(leaf, |branches, _| {
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

fn value_chain(depth: usize, wrap: impl Fn(Value) -> Value) -> Value {
    (0..depth).fold(Value::Bool(true), |inner, _| wrap(inner))
}

fn list_chain(depth: usize) -> Value {
    value_chain(depth, |inner| Value::List(vec![inner]))
}

fn run_on_stack(stack_bytes: usize, depth: usize, work: fn(usize)) {
    std::thread::Builder::new()
        .stack_size(stack_bytes)
        .spawn(move || work(depth))
        .unwrap()
        .join()
        .expect("the chain is built and dropped without panicking");
}

macro_rules! drop_case {
    ($name:ident, $work:expr) => {
        mod $name {
            use super::*;

            #[test]
            fn shallow() {
                run_on_stack(SHALLOW_STACK_BYTES, SHALLOW_DEPTH, $work);
            }

            #[test]
            fn deep() {
                run_on_stack(DEEP_STACK_BYTES, DEEP_DEPTH, $work);
            }
        }
    };
}

drop_case!(standalone_expression, |depth| drop(not_chain(depth)));

drop_case!(standalone_branch, |depth| {
    let branch = frame_chain(depth)
        .pop()
        .expect("the chain holds one branch");
    drop(branch);
});

drop_case!(standalone_step_frame, |depth| {
    drop(Step::Frame {
        arguments: vec![],
        captures: vec![],
        parameter_count: 0,
        capture_count: 0,
        local_scope: 0,
        branches: frame_chain(depth),
    });
});

drop_case!(standalone_temporal_formula, |depth| drop(formula_chain(
    depth
)));

drop_case!(standalone_value_list, |depth| drop(list_chain(depth)));

drop_case!(standalone_value_set, |depth| drop(value_chain(
    depth,
    |inner| Value::Set(vec![inner])
)));

drop_case!(standalone_value_record, |depth| drop(value_chain(
    depth,
    |inner| Value::Record(vec![("field".into(), inner)])
)));

drop_case!(standalone_value_map_key, |depth| drop(value_chain(
    depth,
    |inner| Value::Map(vec![(inner, Value::Bool(true))])
)));

drop_case!(standalone_value_map_value, |depth| drop(value_chain(
    depth,
    |inner| Value::Map(vec![(Value::Bool(true), inner)])
)));

drop_case!(definition_body_assignment, |depth| {
    let mut input = program(not_chain(depth));
    *first_body(&mut input) = truth();
    drop(input);
});

drop_case!(next_branches_clear, |depth| {
    let mut input = program(truth());
    input.next = frame_chain(depth);
    input.next.clear();
    drop(input);
});

drop_case!(data_value_assignment, |depth| {
    let mut input = program(Expression::Data {
        value: list_chain(depth),
    });
    let Expression::Data { value } = first_body(&mut input) else {
        panic!("the body is a data expression")
    };
    *value = Value::Null;
    drop(input);
});

drop_case!(data_list_clear, |depth| {
    let mut input = program(Expression::Data {
        value: list_chain(depth),
    });
    let Expression::Data {
        value: Value::List(items),
    } = first_body(&mut input)
    else {
        panic!("the body is a data list")
    };
    items.clear();
    drop(input);
});

drop_case!(property_formula_replacement, |depth| {
    let mut input = program(truth());
    input.properties = vec![TemporalProperty {
        name: "Deep".into(),
        formula: formula_chain(depth),
    }];
    let replaced = std::mem::replace(&mut input.properties[0].formula, atom());
    drop(replaced);
    drop(input);
});

drop_case!(scalar_refusal, |depth| {
    let refusal = Scalar::try_from(list_chain(depth)).map_err(|error| (error.code, error.message));
    assert_eq!(
        refusal,
        Err((
            "protocol",
            "Safety values must be integers or booleans".to_string()
        ))
    );
});

drop_case!(safety_result_trace_values, |depth| {
    drop(SafetyResult {
        status: "violated".into(),
        reason: None,
        states: 1,
        generated: 1,
        depth: 0,
        work: 0,
        violation: Some(SafetyViolation {
            name: "Deep".into(),
            trace: vec![SafetyTraceStep {
                action: None,
                values: vec![list_chain(depth)],
            }],
            loop_start: None,
            loop_action: None,
        }),
    });
});
