use idealis_connect::{
    ActionEdge, Case, Effect, Generator, Plan, Session, ShrinkStatus, TestStatus, Verdict,
};
use std::collections::BTreeMap;

fn evaluate(case: &Case) -> Verdict {
    let mut durable = false;
    let mut volatile = false;
    let mut running = true;
    let mut lost = false;
    for action in &case.actions {
        match action.as_str() {
            Some("write") if running => {
                durable = true;
                volatile = true;
            }
            Some("crash") => {
                volatile = false;
                running = false;
            }
            Some("restart") => running = true,
            _ => (),
        }
        lost |= running && durable && !volatile;
    }
    if lost {
        Verdict::Mismatch {
            signature: "lost-write".into(),
        }
    } else {
        Verdict::Matched {}
    }
}

fn main() -> Result<(), idealis_connect::Error> {
    let generator = Generator {
        id: "counter".into(),
        version: "1".into(),
        initial: vec!["s".into()],
        states: BTreeMap::from([(
            "s".into(),
            ["write", "crash", "restart", "noise"]
                .into_iter()
                .map(|action| ActionEdge {
                    action: action.into(),
                    target: "s".into(),
                })
                .collect(),
        )]),
        min_length: 0,
        max_length: 24,
    };
    let mut session = Session::property(Plan {
        generator,
        seed: 0x36a1,
        cases: 64,
        max_attempts: 128,
        max_shrink_checks: 128,
        max_work: 100_000,
    })?;
    let case = loop {
        match session.effect() {
            Effect::Evaluate { id, case, .. } => {
                session.resume(id, evaluate(&case))?;
            }
            Effect::Done { result } => {
                assert_eq!(result.status, TestStatus::Failed);
                assert_eq!(result.shrink.unwrap().status, ShrinkStatus::Minimal);
                let case = result.counterexample.unwrap();
                assert_eq!(
                    case.actions
                        .iter()
                        .filter_map(|action| action.as_str())
                        .collect::<Vec<_>>(),
                    vec!["write", "crash", "restart"]
                );
                break case;
            }
        }
    };
    let mut replay = Session::replay(case, Some("lost-write".into()))?;
    let Effect::Evaluate { id, case, .. } = replay.effect() else {
        panic!("replay effect")
    };
    replay.resume(id, evaluate(&case))?;
    let Effect::Done { result } = replay.effect() else {
        panic!("replay result")
    };
    assert_eq!(result.status, TestStatus::Failed);
    println!("standalone idealis-connect consumer checked");
    Ok(())
}
