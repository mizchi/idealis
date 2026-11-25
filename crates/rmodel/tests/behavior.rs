use rmodel::reference::{Machine, Next, Outcome, evaluate};

const SOURCE: &str = include_str!("../../../spec/models/rmodel-behavior.model");
const SWAP: &str = "behavior swap() {
  guard { left = Off right = On }
  update { left = right right = left }
}";

fn source(declaration: &str) -> String {
    assert_eq!(SOURCE.matches(SWAP).count(), 1);
    SOURCE.replacen(SWAP, declaration, 1)
}

#[test]
fn behavior_has_the_same_guard_parallel_updates_and_frame_as_action() {
    let old = rmodel::parse(&source(
        "action swap() { left = Off right = On left' = right right' = left }",
    ))
    .unwrap();
    let new = rmodel::parse(SOURCE).unwrap();

    let verdicts = evaluate(&new, 100);
    assert_eq!(verdicts, evaluate(&old, 100));
    for (name, expected) in [
        ("PreservesUpdate", Outcome::Holds),
        ("Distinct", Outcome::Holds),
        ("Swapped", Outcome::Reachable { depth: 1 }),
        ("BothOn", Outcome::Unreachable),
    ] {
        assert_eq!(
            verdicts.iter().find(|v| v.check == name).unwrap().outcome,
            expected
        );
        assert_eq!(
            rmodel::to_tla(&new, name).unwrap(),
            rmodel::to_tla(&old, name).unwrap()
        );
    }
    assert_eq!(rmodel::to_alloy(&new), rmodel::to_alloy(&old));

    let machine = Machine::new(&new).unwrap();
    let config = machine.configurations().next().unwrap();
    let initial = machine.initial(&config);
    let steps = machine.steps(&config, &initial[0]);
    let Some(Next::State(next)) = &steps[0].next else {
        panic!("expected the swap to be enabled");
    };
    assert!(machine.steps(&config, next)[0].next.is_none());
}

#[test]
fn behavior_checks_update_targets_and_keeps_legacy_identifiers() {
    use rmodel::ErrorKind::{Syntax, Type};

    for (declaration, kind) in [
        ("behavior swap() { update { left = On left = Off } }", Type),
        ("behavior swap() { update { Value = On } }", Type),
        ("behavior swap() { update { left' = On } }", Syntax),
        ("behavior swap() { left = Off }", Syntax),
        (
            "behavior swap() { guard { left = Off } guard { right = On } }",
            Syntax,
        ),
    ] {
        assert_eq!(
            rmodel::parse(&source(declaration)).unwrap_err().kind,
            kind,
            "{declaration}"
        );
    }
    for declaration in [
        "action behavior(guard: Value) { guard != left update' = guard }",
        "behavior swap() { update { left = On } }",
        "behavior swap() { guard { left = Off } }",
        "behavior swap() {}",
    ] {
        assert!(rmodel::parse(&source(declaration)).is_ok(), "{declaration}");
    }
}
