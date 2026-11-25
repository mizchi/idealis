use rcheck::{Budget, Selection, Verdict};

const SHIPMENT: &str = include_str!("../../../fixtures/rcheck/structured/shipped-transition.model");

#[test]
fn shipment_guarantee_checks_every_operation_before_and_after() {
    let faulty = SHIPMENT.replace(
        "acknowledged = true }",
        "acknowledged = true status = Shipped(Post) }",
    );
    for (source, expected, depth) in [
        (SHIPMENT, Verdict::Holds, None),
        (&faulty, Verdict::Violated, Some(2)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let ir = rcheck::ir::to_ir(&model).unwrap();
        let (model, omitted) = rcheck::ir::parse(&serde_json::to_string(&ir).unwrap()).unwrap();
        assert!(omitted.is_empty());
        assert_eq!(rcheck::ir::to_ir(&model).unwrap(), ir);
        let report =
            rcheck::check(&model, "ShippedStays", &Selection::All, Budget::default()).unwrap();
        assert_eq!(report.answers.len(), 2, "{report:?}");
        for answer in report.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (expected.clone(), depth),
                "{}: {:?}",
                answer.checker,
                answer.detail
            );
        }
    }
}

#[test]
fn the_rule_checks_explicit_no_ops_but_ignores_implicit_waiting() {
    let source = "model NoOp
        var done: one Bool
        init { done = false }
        behavior work() { guard { done = false } update { done = true } }
        transition Changes { before(done) != after(done) }
        scope steps = 4";
    let explicit = source.replace(
        "transition Changes",
        "behavior acknowledge() { guard { done = true } } transition Changes",
    );
    for (source, verdict, depth) in [
        (source, Verdict::Holds, None),
        (&explicit, Verdict::Violated, Some(2)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(&model, "Changes", &Selection::All, Budget::default()).unwrap();
        assert_eq!(report.answers.len(), 2, "{report:?}");
        for answer in report.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (verdict.clone(), depth),
                "{:?}",
                answer.detail
            );
        }
    }
}

#[test]
fn now_and_next_are_checked_before_dispatching_the_domain() {
    use rcheck::conform::{ValidationError, Validator};
    use serde_json::json;

    let flawed = SHIPMENT.replace(
        "acknowledged = true }",
        "acknowledged = true status = Shipped(Post) }",
    );
    let model = rmodel::parse(&flawed).unwrap();
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(
            &json!({}),
            &json!({"status":{"tag":"Ready"},"acknowledged":false}),
        )
        .unwrap();
    session
        .replay(&json!({"kind":"ship","method":"Courier"}))
        .unwrap();
    assert_eq!(
        session.dispatch(&json!({"kind":"acknowledge"}), |_| panic!(
            "invalid shipment was dispatched"
        )),
        Err(ValidationError::OutsideModel(
            "transition ShippedStays".into()
        )),
    );
}

#[test]
fn after_syntax_is_rejected_outside_a_transition_and_when_nested() {
    for changed in [
        SHIPMENT.replace("transition ShippedStays", "invariant ShippedStays"),
        SHIPMENT.replace("after(status)", "after(before(status))"),
        SHIPMENT.replace("is(status, Ready)", "is(status', Ready)"),
    ] {
        let error = rmodel::parse(&changed).unwrap_err();
        assert!(
            error.message.contains("only allowed in a transition"),
            "{error}"
        );
    }
}

#[test]
fn an_empty_or_incomplete_graph_cannot_satisfy_a_transition_question() {
    let empty = rmodel::parse(&SHIPMENT.replace(
        "acknowledged = false",
        "acknowledged = false acknowledged = true",
    ))
    .unwrap();
    let valid = rmodel::parse(SHIPMENT).unwrap();
    let incomplete = Budget {
        states: 1,
        ..Budget::default()
    };
    let escaped = rmodel::parse(
        "model Escape
          var v: one Nat
          init { v = 0 }
          behavior tick() { update { v = v + 1 } }
          transition NonDecreasing { after(v) >= before(v) }
          scope Int = 0..0, steps = 4",
    )
    .unwrap();
    for (model, name, budget, expected) in [
        (&empty, "ShippedStays", Budget::default(), Verdict::Empty),
        (&valid, "ShippedStays", incomplete, Verdict::Unknown),
        (
            &escaped,
            "NonDecreasing",
            Budget::default(),
            Verdict::Unknown,
        ),
    ] {
        let report = rcheck::check(model, name, &Selection::All, budget).unwrap();
        assert!(
            report
                .answers
                .iter()
                .all(|answer| answer.verdict == expected),
            "{report:?}"
        );
    }
}
