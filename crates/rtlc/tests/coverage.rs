use rtlc::ir::{Branch, Expression, Scalar, Step};
use rtlc::{SafetyBounds, SafetyProgram};

#[path = "../examples/counter.rs"]
mod counter;

fn bounds() -> SafetyBounds {
    SafetyBounds {
        max_depth: 100,
        max_states: 1000,
        max_work: 1_000_000,
        max_temporal_nodes: Some(10000),
    }
}
fn check(program: &SafetyProgram, limits: SafetyBounds) -> rtlc::SafetyCoverageReport {
    let measured = program.check_with_coverage(limits).unwrap();
    assert_eq!(measured.result, program.check(limits).unwrap());
    let rows = measured
        .coverage
        .initial
        .iter()
        .chain(&measured.coverage.next);
    assert_eq!(
        rows.clone().map(|row| row.generated).sum::<u32>(),
        measured.result.generated
    );
    assert_eq!(
        rows.clone().map(|row| row.distinct).sum::<usize>(),
        measured.result.states
    );
    assert_eq!(
        rows.map(|row| row.work).sum::<u32>() + measured.coverage.unattributed_work,
        measured.result.work
    );
    measured
}
#[test]
fn action_rows_distinguish_attempts_candidates_and_new_states() {
    let measured = check(&counter::ring(), bounds());
    assert_eq!(measured.result.status, "complete");
    let initial = &measured.coverage.initial[0];
    assert_eq!(
        (
            initial.attempts,
            initial.enabled,
            initial.generated,
            initial.distinct
        ),
        (1, 1, 1, 1)
    );
    let next = &measured.coverage.next[0];
    assert_eq!(
        (next.attempts, next.enabled, next.generated, next.distinct),
        (4, 4, 4, 3)
    );
}
#[test]
fn unreachable_and_same_name_branches_remain_separate() {
    let mut program = counter::ring();
    program.next.push(program.next[0].clone());
    program.next.push(Branch {
        name: "Unused".into(),
        steps: vec![Step::Guard {
            expression: Expression::Literal {
                value: Scalar::Bool(false),
            },
        }],
    });
    let measured = check(&program, bounds());
    let rows = &measured.coverage.next;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].name, rows[1].name);
    assert_eq!((rows[0].generated, rows[0].distinct), (4, 3));
    assert_eq!((rows[1].generated, rows[1].distinct), (4, 0));
    assert_eq!(
        (rows[2].attempts, rows[2].enabled, rows[2].generated),
        (4, 0, 0)
    );
    assert!(rows[2].work > 0);
}
#[test]
fn repeated_initial_candidates_are_not_new_states() {
    let mut program = counter::ring();
    program.initial.push(program.initial[0].clone());
    let measured = check(&program, bounds());
    assert_eq!(
        (
            measured.coverage.initial[0].generated,
            measured.coverage.initial[0].distinct
        ),
        (1, 1)
    );
    assert_eq!(
        (
            measured.coverage.initial[1].generated,
            measured.coverage.initial[1].distinct
        ),
        (1, 0)
    );
}
#[test]
fn partial_work_is_accounted_when_a_branch_exhausts_the_budget() {
    let program = counter::ring();
    let measured = check(
        &program,
        SafetyBounds {
            max_work: 1,
            ..bounds()
        },
    );
    assert_eq!(measured.result.status, "bounded");
    assert_eq!(measured.coverage.initial[0].attempts, 1);
    assert_eq!(measured.coverage.initial[0].work, 1);
    assert_eq!(measured.coverage.next[0].attempts, 0);
    for limits in [
        SafetyBounds {
            max_states: 1,
            ..bounds()
        },
        SafetyBounds {
            max_depth: 1,
            ..bounds()
        },
    ] {
        assert_eq!(check(&program, limits).result.status, "bounded");
    }
}
#[test]
fn temporal_work_is_not_attributed_to_the_last_action() {
    let mut encoded = serde_json::to_value(counter::ring()).unwrap();
    encoded["properties"] = serde_json::json!([{"name":"Never","formula":{"kind":"eventually","formula":{"kind":"atom","expression":{"op":"literal","value":{"type":"bool","value":false}}}}}]);
    let program: SafetyProgram = serde_json::from_value(encoded).unwrap();
    let measured = check(&program, bounds());
    assert_eq!(measured.result.status, "liveness-violation");
    assert!(measured.coverage.unattributed_work > 0);
    assert_eq!(
        check(
            &program,
            SafetyBounds {
                max_temporal_nodes: Some(1),
                ..bounds()
            }
        )
        .result
        .status,
        "bounded"
    );
}
#[test]
fn evaluation_errors_are_not_partial_success_reports() {
    let mut program = counter::ring();
    program.initial[0].steps[0] = Step::Assign {
        slot: 0,
        value: Expression::Div {
            left: Box::new(Expression::Literal {
                value: Scalar::Int(1),
            }),
            right: Box::new(Expression::Literal {
                value: Scalar::Int(0),
            }),
        },
    };
    let measured = program.check_with_coverage(bounds()).unwrap_err();
    let normal = program.check(bounds()).unwrap_err();
    assert_eq!(
        (measured.code, measured.message),
        (normal.code, normal.message)
    );
}
