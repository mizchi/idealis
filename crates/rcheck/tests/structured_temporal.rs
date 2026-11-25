use rcheck::{Budget, Coverage, Selection, Verdict};

#[test]
fn fairness_changes_progress_without_changing_reachability() {
    let fair = include_str!("../../../fixtures/rcheck/structured/delivery.model");
    for (source, expected) in [
        (fair.to_owned(), Verdict::Holds),
        (
            fair.replace("fairness weak deliver", "fairness strong deliver"),
            Verdict::Holds,
        ),
        (fair.replace("fairness weak deliver", ""), Verdict::Violated),
    ] {
        let model = rmodel::parse(&source).unwrap();
        let ir = rcheck::ir::to_ir(&model).unwrap();
        let (model, _) = rcheck::ir::parse(&serde_json::to_string(&ir).unwrap()).unwrap();
        assert_eq!(rcheck::ir::to_ir(&model).unwrap(), ir);
        let answers = |name| {
            rcheck::check(&model, name, &Selection::All, Budget::default())
                .unwrap()
                .answers
        };
        assert!(
            answers("CanDeliver")
                .iter()
                .all(|a| a.verdict == Verdict::Reachable)
        );
        let temporal = answers("Delivers");
        assert_eq!(temporal.len(), 1);
        assert_eq!(temporal[0].checker, "explicit");
        assert_eq!(temporal[0].coverage, Coverage::Complete);
        assert_eq!(temporal[0].verdict, expected, "{temporal:?}");
    }
}

#[test]
fn structured_values_round_trip_and_have_the_same_verdict_in_the_explicit_checker() {
    let source = include_str!("../../../fixtures/rcheck/structured/ordered-log.model");
    let model = rmodel::parse(source).unwrap();
    let ir = rcheck::ir::to_ir(&model).unwrap();
    let (model, omitted) = rcheck::ir::parse(&serde_json::to_string(&ir).unwrap()).unwrap();
    assert!(omitted.is_empty());
    assert_eq!(rcheck::ir::to_ir(&model).unwrap(), ir);

    for (name, verdict, depth) in [
        ("ValidResult", Verdict::Holds, None),
        ("Bounded", Verdict::Holds, None),
        ("Ordered", Verdict::Reachable, Some(2)),
        ("Repeated", Verdict::Reachable, Some(2)),
        ("Missing", Verdict::Reachable, Some(0)),
    ] {
        let all = rcheck::check(&model, name, &Selection::All, Budget::default()).unwrap();
        assert_eq!(all.answers.len(), 2, "{all:?}");
        for answer in all.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (verdict.clone(), depth),
                "{} {name}: {:?}",
                answer.checker,
                answer.detail
            );
        }
    }
}

#[test]
fn runtime_validation_preserves_sequence_order_and_checks_tags() {
    use rcheck::conform::{ValidationError, Validator};
    use serde_json::json;

    let source = include_str!("../../../fixtures/rcheck/structured/ordered-log.model");
    let model = rmodel::parse(source).unwrap();
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(
            &json!({}),
            &json!({"log":{"$sequence":[]},"result":{"tag":"Pending"}}),
        )
        .unwrap();
    session.replay(&json!({"kind":"add","e":"A"})).unwrap();
    let command = json!({"kind":"add","e":"B"});
    assert_eq!(
        session.validate(&command).unwrap(),
        json!({"log":{"$sequence":["A","B"]},"result":{"tag":"Added","value":"B"}})
    );
    let wrong = json!({"log":{"$sequence":["B","A"]},"result":{"tag":"Added","value":"B"}});
    assert!(matches!(
        session.dispatch(&command, |_| Ok((true, wrong))),
        Err(ValidationError::Domain(_))
    ));

    for invalid in [
        json!({"log":{"$sequence":["A","B","A"]},"result":{"tag":"Pending"}}),
        json!({"log":{"$sequence":[]},"result":{"tag":"Added"}}),
        json!({"log":{"$sequence":[]},"result":{"tag":"Pending","value":"B"}}),
    ] {
        assert!(matches!(
            validator.start(&json!({}), &invalid),
            Err(ValidationError::InvalidInput(_))
        ));
    }
}

#[test]
fn append_past_the_bound_is_not_a_successful_safety_check() {
    let source = include_str!("../../../fixtures/rcheck/structured/ordered-log.model")
        .replace("guard { len(log) < 2 }", "");
    let model = rmodel::parse(&source).unwrap();
    assert_eq!(
        rmodel::reference::analyze(&model, 1_000).scope,
        rmodel::reference::Outcome::Violated { depth: 3 }
    );
    let all = rcheck::check(&model, "Bounded", &Selection::All, Budget::default()).unwrap();
    assert!(
        all.answers.iter().all(|a| a.verdict == Verdict::Unknown),
        "{all:?}"
    );
}

#[test]
fn strong_fairness_handles_a_delivery_enabled_only_intermittently() {
    let strong = include_str!("../../../fixtures/rcheck/structured/retry-delivery.model");
    for (source, expected) in [
        (strong.to_owned(), Verdict::Holds),
        (
            strong.replace("fairness strong deliver", "fairness weak deliver"),
            Verdict::Violated,
        ),
    ] {
        let model = rmodel::parse(&source).unwrap();
        let answers = rcheck::check(&model, "Delivers", &Selection::All, Budget::default())
            .unwrap()
            .answers;
        assert_eq!(answers[0].verdict, expected, "{answers:?}");
        if expected == Verdict::Violated {
            let lasso = answers[0]
                .lasso
                .as_ref()
                .expect("counterexample with a cycle");
            assert!(lasso["loopStart"].is_number());
            assert!(lasso["trace"].as_array().unwrap().len() >= 2);
        }
    }
}

#[test]
fn empty_or_budget_limited_progress_does_not_pass() {
    let fair = include_str!("../../../fixtures/rcheck/structured/delivery.model");
    let empty = rmodel::parse(&fair.replace(
        "init { status = Waiting }",
        "init { status = Waiting status = Delivered }",
    ))
    .unwrap();
    let out = rcheck::check(&empty, "Delivers", &Selection::All, Budget::default()).unwrap();
    assert_eq!(out.answers[0].verdict, Verdict::Empty);

    let fair = rmodel::parse(fair).unwrap();
    let budget = Budget {
        states: 1,
        ..Budget::default()
    };
    let out = rcheck::check(&fair, "Delivers", &Selection::All, budget).unwrap();
    assert_eq!(out.answers[0].verdict, Verdict::Unknown);
}

#[test]
fn new_declarations_reject_wrong_payloads_and_recursive_types() {
    let good = include_str!("../../../fixtures/rcheck/structured/ordered-log.model");
    for (source, message) in [
        (good.replace("Added(e)", "Added()"), "payload"),
        (good.replace("Added(e)", "Added(log)"), "one Entry"),
        (
            good.replace("sequence Journal = Entry", "sequence Journal = Result")
                .replace("Added(Entry)", "Added(Journal)"),
            "recursive value type",
        ),
        (
            good.replace("Journal[A, B]", "Journal[A, B, A]"),
            "literal exceeds max",
        ),
    ] {
        assert!(
            rmodel::parse(&source)
                .unwrap_err()
                .message
                .contains(message),
            "{source}"
        );
    }
}

#[test]
fn a_payment_can_carry_a_numeric_record_or_a_rejection_reason() {
    use rcheck::conform::{ValidationError, Validator};
    use serde_json::json;

    let model = rmodel::parse(include_str!(
        "../../../fixtures/rcheck/structured/payment.model"
    ))
    .unwrap();
    for (name, expected) in [
        ("NoZeroPayment", Verdict::Holds),
        ("PaidExactly", Verdict::Reachable),
        ("CanReject", Verdict::Reachable),
    ] {
        let all = rcheck::check(&model, name, &Selection::All, Budget::default()).unwrap();
        assert_eq!(all.answers.len(), 2, "{all:?}");
        assert!(all.answers.iter().all(|a| a.verdict == expected), "{all:?}");
    }

    let validator = Validator::new(&model).unwrap();
    let session = validator
        .start(&json!({}), &json!({"result":{"tag":"Pending"}}))
        .unwrap();
    let complete = |outcome| json!({"kind":"complete","outcome":outcome});
    assert_eq!(
        session
            .validate(&complete(json!({"tag":"Paid","value":{"units":2}})))
            .unwrap(),
        json!({"result":{"tag":"Paid","value":{"units":2}}})
    );
    assert!(matches!(
        session.validate(&complete(json!({"tag":"Paid", "value":"Declined"}))),
        Err(ValidationError::InvalidInput(_))
    ));
    assert!(matches!(
        session.validate(&complete(json!({"tag":"Paid", "value":{"units":0}}))),
        Err(ValidationError::Disabled(_))
    ));
}

#[test]
fn integer_boundaries_inside_a_sequence_keep_the_other_elements_fixed() {
    let model = rmodel::parse(
        "model SequenceInput
         sequence Values = Nat, max = 2
         var active: one Bool
         init { active = false }
         behavior accept(values: Values) {
           guard { len(values) = 2 at(values, 1) = 1 }
           update { active = true }
         }
         scope Int = 0..1, steps = 3",
    )
    .unwrap();
    let cases = rcheck::conform::boundary_cases(&model, 1_000).unwrap();
    let pair = cases["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pair| pair["kind"] == "integer")
        .unwrap();
    assert_eq!(
        pair["examples"][0]["command"]["values"]["$sequence"],
        serde_json::json!([0, 0])
    );
    assert_eq!(
        pair["examples"][1]["command"]["values"]["$sequence"],
        serde_json::json!([0, 1])
    );
}
