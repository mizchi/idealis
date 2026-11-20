use idealis_connect::{Effect, Generator, Plan, Session, ShrinkStatus, TestStatus, Verdict};
use serde_json::json;

fn generator() -> Generator {
    serde_json::from_value(json!({"id":"counter-actions","version":"1","initial":["ready"],"states":{"ready":[{"action":"noise","target":"ready"},{"action":"add","target":"ready"},{"action":"crash","target":"ready"},{"action":"restart","target":"ready"}]},"minLength":0,"maxLength":24})).unwrap()
}
fn plan() -> Plan {
    Plan {
        generator: generator(),
        seed: 0x36a1,
        cases: 64,
        max_attempts: 128,
        max_shrink_checks: 128,
        max_work: 100_000,
    }
}
fn case(actions: serde_json::Value) -> idealis_connect::Case {
    serde_json::from_value(json!({"format":"idealis.test-case","version":1,"rng":"xorshift32-v1","generatorId":"counter-actions","generatorVersion":"1","seed":1,"index":0,"actions":actions})).unwrap()
}
fn evaluate(actions: &[serde_json::Value]) -> Verdict {
    let mut value = 0;
    let mut acknowledged = 0;
    let mut running = true;
    for action in actions {
        match action.as_str().unwrap() {
            "add" if running => {
                value += 1;
                acknowledged += 1;
            }
            "crash" => {
                running = false;
                value = 0;
            }
            "restart" => running = true,
            _ => (),
        }
        if running && value != acknowledged {
            return Verdict::Mismatch {
                signature: "lost-write".into(),
            };
        }
    }
    Verdict::Matched {}
}
fn run(
    session: &mut Session,
    evaluate: impl Fn(&[serde_json::Value]) -> Verdict,
) -> idealis_connect::TestResult {
    loop {
        match session.effect() {
            Effect::Evaluate { id, case, .. } => {
                session.resume(id, evaluate(&case.actions)).unwrap()
            }
            Effect::Done { result } => return result,
        }
    }
}

#[test]
fn generates_shrinks_confirms_and_replays_a_real_stateful_defect() {
    let mut session = Session::property(plan()).unwrap();
    let result = run(&mut session, evaluate);
    assert_eq!(result.status, TestStatus::Failed);
    assert_eq!(
        result.shrink.as_ref().unwrap().status,
        ShrinkStatus::Minimal
    );
    let case = result.counterexample.unwrap();
    assert_eq!(
        case.actions,
        vec![json!("add"), json!("crash"), json!("restart")]
    );
    let encoded = serde_json::to_string(&case).unwrap();
    let restored = serde_json::from_str(&encoded).unwrap();
    let result = run(
        &mut Session::replay(restored, Some("lost-write".into())).unwrap(),
        evaluate,
    );
    assert_eq!(result.status, TestStatus::Failed);
    assert_eq!(result.trials.len(), 1);
}

#[test]
fn passing_requires_the_requested_number_of_valid_cases() {
    let result = run(&mut Session::property(plan()).unwrap(), |_| {
        Verdict::Matched {}
    });
    assert_eq!(result.status, TestStatus::Passed);
    assert_eq!(result.cases, 64);
    assert_eq!(result.trials.len(), 64);
    let result = run(&mut Session::property(plan()).unwrap(), |_| {
        Verdict::Invalid {}
    });
    assert_eq!(result.status, TestStatus::Exhausted);
    assert_eq!(result.cases, 0);
    assert_eq!(result.attempts, 128);
    for verdict in [
        Verdict::Unknown {
            reason: "solver".into(),
        },
        Verdict::Error {
            message: "driver".into(),
        },
    ] {
        let result = run(&mut Session::property(plan()).unwrap(), |_| verdict.clone());
        assert!(result.status == TestStatus::Inconclusive || result.status == TestStatus::Error);
    }
}

#[test]
fn work_exhaustion_and_dead_end_generation_never_pass() {
    let mut input = plan();
    input.max_work = 0;
    let Effect::Done { result } = Session::property(input).unwrap().effect() else {
        panic!("work exhausted")
    };
    assert_eq!(result.status, TestStatus::Inconclusive);
    let mut input = plan();
    input.generator.min_length = 1;
    input.generator.states.get_mut("ready").unwrap().clear();
    let result = run(&mut Session::property(input).unwrap(), |_| {
        panic!("no generated case")
    });
    assert_eq!(result.status, TestStatus::Exhausted);
}

#[test]
fn feedback_correlation_is_checked_without_consuming_the_request() {
    let mut session = Session::property(plan()).unwrap();
    let before = serde_json::to_value(session.effect()).unwrap();
    assert!(session.resume(999, Verdict::Matched {}).is_err());
    assert_eq!(serde_json::to_value(session.effect()).unwrap(), before);
    let Effect::Evaluate { id, .. } = session.effect() else {
        panic!("pending")
    };
    assert!(
        session
            .resume(
                id,
                Verdict::Mismatch {
                    signature: "".into()
                }
            )
            .is_err()
    );
    assert_eq!(serde_json::to_value(session.effect()).unwrap(), before);
}

#[test]
fn a_nonreproducing_failure_is_not_reported_as_confirmed() {
    let mut input = plan();
    input.max_shrink_checks = 2;
    let mut session = Session::property(input).unwrap();
    let Effect::Evaluate { id, .. } = session.effect() else {
        panic!("pending")
    };
    session
        .resume(
            id,
            Verdict::Mismatch {
                signature: "original".into(),
            },
        )
        .unwrap();
    let result = run(&mut session, |_| Verdict::Matched {});
    assert_eq!(result.status, TestStatus::Error);
    assert_eq!(result.reason.as_deref(), Some("not-reproduced"));
}

#[test]
fn unknown_shrink_candidates_remain_bounded_even_when_empty_fails() {
    let mut session =
        Session::shrink(generator(), case(json!(["noise", "noise"])), 16, 10_000).unwrap();
    for index in 0..8 {
        match session.effect() {
            Effect::Evaluate { id, .. } => session
                .resume(
                    id,
                    if index == 1 {
                        Verdict::Unknown {
                            reason: "work".into(),
                        }
                    } else {
                        Verdict::Mismatch {
                            signature: "same".into(),
                        }
                    },
                )
                .unwrap(),
            Effect::Done { result } => {
                assert_eq!(result.status, TestStatus::Failed);
                assert_eq!(result.shrink.unwrap().status, ShrinkStatus::Bounded);
                return;
            }
        }
    }
    panic!("shrinking did not finish within its candidates");
}

#[test]
fn shrinking_preserves_state_machine_preconditions_and_failure_signature() {
    let mut input = generator();
    input.initial = vec!["new".into()];
    input.states = serde_json::from_value(json!({"new":[{"action":"setup","target":"ready"}],"ready":[{"action":"noise","target":"ready"},{"action":"read","target":"ready"}]})).unwrap();
    let mut session = Session::shrink(
        input,
        case(json!(["setup", "noise", "read", "noise"])),
        64,
        10_000,
    )
    .unwrap();
    let result = run(&mut session, |actions| {
        if !actions.is_empty() {
            assert_eq!(actions.first(), Some(&json!("setup")));
        }
        if actions.contains(&json!("read")) {
            Verdict::Mismatch {
                signature: "read".into(),
            }
        } else {
            Verdict::Mismatch {
                signature: "different".into(),
            }
        }
    });
    assert_eq!(result.status, TestStatus::Failed);
    assert_eq!(
        result.counterexample.unwrap().actions,
        vec![json!("setup"), json!("read")]
    );
}
