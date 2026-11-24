#![allow(dead_code)]

use rtlc::ir::{
    Binding, BodyKind, Branch, Definition, Domain, Expression, Predicate, Scalar, Step,
};
use rtlc::{SafetyBounds, SafetyProgram};

pub fn int(value: i32) -> Expression {
    Expression::Literal {
        value: Scalar::Int(value),
    }
}

pub fn boolean(value: bool) -> Expression {
    Expression::Literal {
        value: Scalar::Bool(value),
    }
}

pub fn x() -> Expression {
    Expression::Read {
        slot: 0,
        next: false,
    }
}

pub fn bound(slot: usize) -> Expression {
    Expression::Bound { slot }
}

pub fn parameter(slot: usize) -> Expression {
    Expression::Parameter { slot }
}

fn boxed(value: Expression) -> Box<Expression> {
    Box::new(value)
}

pub fn eq(left: Expression, right: Expression) -> Expression {
    Expression::Eq {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn ne(left: Expression, right: Expression) -> Expression {
    Expression::Ne {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn ge(left: Expression, right: Expression) -> Expression {
    Expression::Ge {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn lt(left: Expression, right: Expression) -> Expression {
    Expression::Lt {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn add(left: Expression, right: Expression) -> Expression {
    Expression::Add {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn sub(left: Expression, right: Expression) -> Expression {
    Expression::Sub {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn div(left: Expression, right: Expression) -> Expression {
    Expression::Div {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn not(argument: Expression) -> Expression {
    Expression::Not {
        argument: boxed(argument),
    }
}

pub fn and(left: Expression, right: Expression) -> Expression {
    Expression::And {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn or(left: Expression, right: Expression) -> Expression {
    Expression::Or {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn implies(left: Expression, right: Expression) -> Expression {
    Expression::Implies {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn if_then(
    condition: Expression,
    consequent: Expression,
    alternative: Expression,
) -> Expression {
    Expression::If {
        condition: boxed(condition),
        consequent: boxed(consequent),
        alternative: boxed(alternative),
    }
}

pub fn call(definition: usize, arguments: Vec<Expression>) -> Expression {
    Expression::Call {
        definition,
        arguments,
        captures: vec![],
    }
}

pub fn range(lower: i32, upper: i32) -> Domain {
    Domain::Range {
        lower: boxed(int(lower)),
        upper: boxed(int(upper)),
    }
}

pub fn function(slot: usize, domain: Domain, body: Expression) -> Expression {
    Expression::Function {
        bounds: vec![Binding {
            tuple_size: None,
            slot,
            domain,
        }],
        body: boxed(body),
    }
}

pub fn at(target: Expression, key: Expression) -> Expression {
    Expression::At {
        target: boxed(target),
        key: boxed(key),
    }
}

pub fn set(values: Vec<Expression>) -> Expression {
    Expression::Set { values }
}

pub fn function_in(left: Expression, right: Expression) -> Expression {
    Expression::FunctionIn {
        left: boxed(left),
        right: boxed(right),
    }
}

pub fn conjunction(terms: Vec<Expression>) -> Expression {
    left_fold(terms, and)
}

pub fn disjunction(terms: Vec<Expression>) -> Expression {
    left_fold(terms, or)
}

pub fn left_fold(
    terms: Vec<Expression>,
    join: fn(Expression, Expression) -> Expression,
) -> Expression {
    let mut terms = terms.into_iter();
    let first = terms.next().expect("a chain has a first term");
    terms.fold(first, join)
}

pub fn right_fold(
    terms: Vec<Expression>,
    join: fn(Expression, Expression) -> Expression,
) -> Expression {
    let mut terms = terms.into_iter().rev();
    let last = terms.next().expect("a chain has a last term");
    terms.fold(last, |right, left| join(left, right))
}

pub fn else_if(links: Vec<(Expression, Expression)>, otherwise: Expression) -> Expression {
    links
        .into_iter()
        .rev()
        .fold(otherwise, |alternative, (condition, consequent)| {
            if_then(condition, consequent, alternative)
        })
}

pub fn definition(id: usize, parameter_count: usize, body: Expression) -> Definition {
    Definition {
        recursive: false,
        id,
        parameter_count,
        capture_count: 0,
        body_kind: BodyKind::Expression,
        local_scope: 0,
        body: body.into(),
    }
}

pub fn recursive(id: usize, parameter_count: usize, body: Expression) -> Definition {
    Definition {
        recursive: true,
        ..definition(id, parameter_count, body)
    }
}

pub fn counter(version: u32, invariant: Expression, definitions: Vec<Definition>) -> SafetyProgram {
    SafetyProgram {
        format: "modelt.safety-program".into(),
        version,
        name: "Chains".into(),
        variables: vec!["x".into()],
        initial: vec![Branch {
            name: "Init".into(),
            steps: vec![Step::Assign {
                slot: 0,
                value: int(0),
            }],
        }],
        next: vec![Branch {
            name: "Next".into(),
            steps: vec![Step::Assign {
                slot: 0,
                value: if_then(lt(x(), int(5)), add(x(), int(1)), int(0)),
            }],
        }],
        invariants: vec![Predicate {
            name: "Inv".into(),
            expression: invariant,
        }],
        assumptions: vec![],
        check_deadlock: false,
        symmetry: None,
        initial_properties: vec![],
        action_properties: vec![],
        properties: vec![],
        fairness: vec![],
        constraints: vec![],
        action_constraints: vec![],
        definitions: (version == 2).then_some(definitions),
    }
}

pub fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 1000,
        max_work: 1_000_000,
        max_temporal_nodes: None,
    }
}

pub fn outcome(program: &SafetyProgram) -> String {
    outcome_within(program, bounds())
}

pub fn outcome_within(program: &SafetyProgram, bounds: SafetyBounds) -> String {
    match program.check(bounds) {
        Ok(result) => format!(
            "{} reason={} states={} generated={} work={} trace={}",
            result.status,
            result.reason.as_deref().unwrap_or("-"),
            result.states,
            result.generated,
            result.work,
            result.violation.map_or_else(
                || "-".to_string(),
                |violation| violation.trace.len().to_string()
            ),
        ),
        Err(error) => format!("error {}: {}", error.code, error.message),
    }
}

pub fn on_large_stack<T: Send + 'static>(body: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(body)
        .expect("spawn")
        .join()
        .expect("join")
}
