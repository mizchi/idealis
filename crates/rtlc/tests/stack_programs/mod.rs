#![allow(dead_code)]

use rtlc::ir::{
    Binding, BodyKind, Branch, Definition, Domain, Expression, Predicate, Scalar, Step,
    TemporalFormula, TemporalProperty,
};
use rtlc::{SafetyBounds, SafetyProgram};

pub const KINDS: [&str; 9] = [
    "operator",
    "value",
    "call",
    "membership",
    "binding",
    "enabled",
    "bind-run",
    "lazy",
    "temporal",
];

const PER_DEFINITION: usize = 30;

fn int(value: i32) -> Expression {
    Expression::Literal {
        value: Scalar::Int(value),
    }
}

fn truth() -> Expression {
    Expression::Literal {
        value: Scalar::Bool(true),
    }
}

fn x() -> Expression {
    Expression::Read {
        slot: 0,
        next: false,
    }
}

fn call(definition: usize) -> Expression {
    Expression::Call {
        definition,
        arguments: vec![],
        captures: vec![],
    }
}

fn singleton(value: Expression) -> Expression {
    Expression::Set {
        values: vec![value],
    }
}

fn definition(id: usize, local_scope: usize, body: Expression) -> Definition {
    Definition {
        recursive: false,
        id,
        parameter_count: 0,
        capture_count: 0,
        body_kind: BodyKind::Expression,
        local_scope,
        body: body.into(),
    }
}

fn layered(
    size: usize,
    per: usize,
    leaf: Expression,
    layer: impl Fn(usize, Expression) -> Expression,
) -> (Expression, Vec<Definition>) {
    let count = size.div_ceil(per).max(1);
    let definitions = (0..count)
        .map(|id| {
            let levels = if id + 1 == count {
                size - per * (count - 1)
            } else {
                per
            };
            let inner = if id + 1 == count {
                leaf.clone()
            } else {
                call(id + 1)
            };
            definition(
                id,
                0,
                (0..levels).fold(inner, |inner, level| layer(level, inner)),
            )
        })
        .collect();
    (call(0), definitions)
}

fn bindings(count: usize, body: Expression) -> Expression {
    Expression::Forall {
        bounds: (0..count)
            .map(|slot| Binding {
                tuple_size: None,
                slot,
                domain: Domain::Set {
                    values: vec![int(1)],
                },
            })
            .collect(),
        body: Box::new(body),
    }
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

fn chained(
    size: usize,
    local_scope: usize,
    body: impl Fn(usize, Expression) -> Expression,
) -> (Expression, Vec<Definition>) {
    let count = size.div_ceil(PER_DEFINITION).max(1);
    let definitions = (0..count)
        .map(|id| {
            let levels = if id + 1 == count {
                size - PER_DEFINITION * (count - 1)
            } else {
                PER_DEFINITION
            };
            let inner = if id + 1 == count {
                truth()
            } else {
                call(id + 1)
            };
            definition(id, local_scope, body(levels, inner))
        })
        .collect();
    (call(0), definitions)
}

fn invariant(size: usize, kind: &str) -> (Expression, Vec<Definition>) {
    match kind {
        "operator" => {
            let (root, definitions) =
                layered(size, PER_DEFINITION, x(), |_, inner| Expression::Neg {
                    argument: Box::new(inner),
                });
            (
                Expression::Ne {
                    left: Box::new(root),
                    right: Box::new(int(1000)),
                },
                definitions,
            )
        }
        "value" => {
            let (root, definitions) = layered(size, PER_DEFINITION / 2, int(1), |_, inner| {
                Expression::Cardinality {
                    argument: Box::new(singleton(inner)),
                }
            });
            (
                Expression::Ge {
                    left: Box::new(root),
                    right: Box::new(int(0)),
                },
                definitions,
            )
        }
        "call" => layered(size, 1, truth(), |_, inner| inner),
        "membership" => {
            let (root, definitions) =
                layered(size, PER_DEFINITION, singleton(truth()), |_, inner| {
                    Expression::SequenceSet {
                        argument: Box::new(inner),
                    }
                });
            (
                Expression::SequenceIn {
                    left: Box::new(Expression::Tuple { values: vec![] }),
                    right: Box::new(root),
                },
                definitions,
            )
        }
        "binding" => chained(size, PER_DEFINITION, bindings),
        "enabled" => chained(size, 0, |levels, inner| {
            let mut steps: Vec<Step> = (1..levels.max(1))
                .map(|_| Step::Guard {
                    expression: truth(),
                })
                .collect();
            steps.push(Step::Guard { expression: inner });
            enabled(steps)
        }),
        "bind-run" => chained(size, PER_DEFINITION, |levels, inner| {
            let mut steps: Vec<Step> = (0..levels)
                .map(|slot| Step::Bind {
                    slot,
                    domain: Domain::Set {
                        values: vec![int(1)],
                    },
                    tuple_size: None,
                })
                .collect();
            steps.push(Step::Guard { expression: inner });
            enabled(steps)
        }),
        "lazy" => {
            let (root, definitions) =
                layered(size, PER_DEFINITION / 2, x(), |_, inner| Expression::At {
                    target: Box::new(Expression::Tuple {
                        values: vec![inner],
                    }),
                    key: Box::new(int(1)),
                });
            (
                Expression::Ge {
                    left: Box::new(root),
                    right: Box::new(int(0)),
                },
                definitions,
            )
        }
        _ => panic!("kind {kind}"),
    }
}

fn temporal(size: usize) -> (TemporalFormula, Vec<Definition>) {
    let count = size.div_ceil(PER_DEFINITION).max(1);
    let definitions = (0..count)
        .map(|id| {
            let levels = if id + 1 == count {
                size - PER_DEFINITION * (count - 1)
            } else {
                PER_DEFINITION
            };
            let inner = if id + 1 == count {
                TemporalFormula::Atom {
                    expression: truth(),
                }
            } else {
                TemporalFormula::Call {
                    definition: id + 1,
                    arguments: vec![],
                    captures: vec![],
                }
            };
            let body = (0..levels).fold(inner, |inner, _| TemporalFormula::Always {
                formula: Box::new(inner),
            });
            Definition {
                recursive: false,
                id,
                parameter_count: 0,
                capture_count: 0,
                body_kind: BodyKind::Temporal,
                local_scope: 0,
                body: temporal_body(body),
            }
        })
        .collect();
    (
        TemporalFormula::Call {
            definition: 0,
            arguments: vec![],
            captures: vec![],
        },
        definitions,
    )
}

pub fn temporal_body(formula: TemporalFormula) -> rtlc::ir::DefinitionBody {
    rtlc::ir::DefinitionBody::Temporal(Box::new(formula))
}

pub fn program(kind: &str, size: usize) -> SafetyProgram {
    let (invariants, properties, definitions) = if kind == "temporal" {
        let (formula, definitions) = temporal(size);
        (
            vec![],
            vec![TemporalProperty {
                name: "Live".into(),
                formula,
            }],
            definitions,
        )
    } else {
        let (expression, definitions) = invariant(size, kind);
        (
            vec![Predicate {
                name: "Inv".into(),
                expression,
            }],
            vec![],
            definitions,
        )
    };
    SafetyProgram {
        format: "modelt.safety-program".into(),
        version: 2,
        name: "Stack".into(),
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
                value: int(0),
            }],
        }],
        invariants,
        assumptions: vec![],
        check_deadlock: false,
        symmetry: None,
        initial_properties: vec![],
        action_properties: vec![],
        properties,
        fairness: vec![],
        constraints: vec![],
        action_constraints: vec![],
        definitions: Some(definitions),
    }
}

pub fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 10,
        max_states: 100,
        max_work: 1_000_000,
        max_temporal_nodes: Some(100_000),
    }
}

pub fn outcome(program: &SafetyProgram) -> String {
    match program.check(bounds()) {
        Ok(result) => format!(
            "{} reason={} states={} generated={} work={}",
            result.status,
            result.reason.as_deref().unwrap_or("-"),
            result.states,
            result.generated,
            result.work,
        ),
        Err(error) => format!("error {}: {}", error.code, error.message),
    }
}

pub fn on_stack(bytes: usize, program: SafetyProgram) -> String {
    std::thread::Builder::new()
        .stack_size(bytes)
        .spawn(move || {
            let result = outcome(&program);
            std::mem::forget(program);
            result
        })
        .expect("spawn")
        .join()
        .expect("join")
}
