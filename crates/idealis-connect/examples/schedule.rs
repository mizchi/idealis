use idealis_connect::{Effect, Plan, Schedule, ScheduleActor, Session, TestStatus, Verdict};

fn main() -> Result<(), idealis_connect::Error> {
    let generator = Schedule {
        id: "consumer-schedule".into(),
        version: "1".into(),
        actors: vec![
            ScheduleActor {
                id: "left".into(),
                operations: vec!["read".into(), "write".into()],
            },
            ScheduleActor {
                id: "right".into(),
                operations: vec!["read".into(), "write".into()],
            },
        ],
    }
    .generator()?;
    assert_eq!(generator.states.len(), 9);
    assert_eq!(generator.max_length, 4);
    let mut session = Session::property(Plan {
        generator,
        seed: 0x3801,
        cases: 4,
        max_attempts: 4,
        max_shrink_checks: 16,
        max_work: 1000,
    })?;
    loop {
        match session.effect() {
            Effect::Evaluate { id, .. } => session.resume(id, Verdict::Matched {})?,
            Effect::Done { result } => {
                assert_eq!(result.status, TestStatus::Passed);
                break;
            }
        }
    }
    println!("standalone schedule generator checked");
    Ok(())
}
