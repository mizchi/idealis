use rcheck::{Budget, Selection, Verdict};

const SHIPMENT: &str = include_str!("../../../fixtures/rcheck/structured/shipped-transition.model");

fn assert_atom_transition_answers(report: &rcheck::Report, verdict: Verdict, depth: Option<usize>) {
    assert_eq!(report.answers.len(), 4, "{report:?}");
    for answer in &report.answers {
        assert_eq!(answer.verdict, verdict, "{answer:?}");
        let expected_depth = match answer.coverage {
            rcheck::Coverage::Complete => depth,
            rcheck::Coverage::Bounded { .. } => None,
        };
        assert_eq!(answer.depth, expected_depth, "{answer:?}");
    }
}

#[test]
fn sat_checks_publication_against_its_preceding_graph() {
    use rcheck::Coverage;
    let source =
        include_str!("../../../examples/model-patterns/worker-transfer/graph-publication.model");
    let faulty = source.replace(
        "if { capturedAt = generation } then {\n      phase = Done",
        "if { phase = Checked } then {\n      phase = Done",
    );
    assert_ne!(faulty, source);
    for (source, expected) in [
        (source, Verdict::Holds),
        (faulty.as_str(), Verdict::Violated),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "PublishedAgainstCurrentGraph",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        let sat = report
            .answers
            .iter()
            .find(|answer| answer.checker == "sat")
            .expect("SAT answers the transition");
        assert_eq!(sat.coverage, Coverage::Bounded { steps: 8 });
        assert_eq!(sat.verdict, expected, "{report:?}");
        assert!(!report.disagreement, "{report:?}");
    }
}

#[test]
fn sat_checks_every_matching_argument_and_only_the_named_action() {
    let source = "model Calls
        enum Choice = Allowed | Other
        var ready: one Bool
        init { not ready }
        behavior selected(choice: Choice) {
          guard { not ready choice = Allowed }
          update { ready = true }
        }
        behavior alias(choice: Choice) { guard { not ready } update { ready = true } }
        transition Checked on selected { choice = Allowed and not before(ready) and after(ready) }
        scope steps = 3";
    let faulty = source.replace("not ready choice = Allowed", "not ready");
    for (source, expected) in [
        (source, Verdict::Holds),
        (faulty.as_str(), Verdict::Violated),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(&model, "Checked", &Selection::All, Budget::default()).unwrap();
        assert_eq!(report.answers.len(), 4, "{report:?}");
        assert!(
            report
                .answers
                .iter()
                .all(|answer| answer.verdict == expected),
            "{report:?}"
        );
    }
}

#[test]
fn sat_distinguishes_an_empty_graph_and_a_multiplicity_violation() {
    let source = "model Clear
        enum Token = A
        var value: one Token
        init { value = A }
        behavior clear() { update { value = none } }
        transition Keep on clear { unchanged(value) }
        scope steps = 3";
    let model = rmodel::parse(source).unwrap();
    let check = |model, question| {
        rcheck::check(
            model,
            question,
            &Selection::Named("sat".into()),
            Budget::default(),
        )
        .unwrap()
        .answers[0]
            .verdict
            .clone()
    };
    assert_eq!(check(&model, "Keep"), Verdict::Holds);
    assert_eq!(check(&model, "Declarations"), Verdict::Violated);
    let empty = rmodel::parse(&source.replace("init { value = A }", "init { no value }")).unwrap();
    assert_eq!(check(&empty, "Keep"), Verdict::Empty);
}

#[test]
fn a_wide_transition_count_does_not_disable_state_checks() {
    let model = rmodel::parse(
        "model WideTransition
         sig A
         sig B
         sig C
         var values: set A
         init { no values }
         behavior fill() { update { values = A } }
         invariant WithinA { values in A }
         transition Nonempty on fill { #after(values) > 0 }
         scope A = 7, B = 7, C = 2, steps = 3",
    )
    .unwrap();
    let bounded = Selection::Named("sat".into());
    let budget = Budget::default();
    assert_eq!(
        rcheck::check(&model, "WithinA", &bounded, budget)
            .unwrap()
            .answers[0]
            .verdict,
        Verdict::Holds
    );
    let error = rcheck::check(&model, "Nonempty", &bounded, budget).unwrap_err();
    assert!(
        matches!(error, rcheck::Error::Unsupported(_, reasons) if reasons[0].1.contains("15 atoms"))
    );
    assert_eq!(
        rcheck::check(&model, "Nonempty", &Selection::Auto, budget)
            .unwrap()
            .answers[0]
            .verdict,
        Verdict::Holds
    );
}

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
        assert_eq!(report.answers.len(), 3, "{report:?}");
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
        assert_atom_transition_answers(&report, verdict, depth);
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

#[test]
fn retry_properties_apply_to_the_selected_call_with_its_argument() {
    use rcheck::conform::{ValidationError, Validator};
    use serde_json::json;

    let source = include_str!("../../../examples/model-patterns/retry/delivery.model");
    let faulty = source.replace(
        "then effects else",
        "then (effects - request -> Effect) + request -> Duplicate else",
    );
    let noop = source.replace(
        "request in before(receipts) implies unchanged(request.effects)",
        "request in before(replies) implies not unchanged(replies)",
    );
    for (source, verdict, depth) in [
        (source, Verdict::Holds, None),
        (faulty.as_str(), Verdict::Violated, Some(2)),
        (noop.as_str(), Verdict::Violated, Some(2)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "RetryPreservesEffect",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_atom_transition_answers(&report, verdict, depth);
    }

    let model = rmodel::parse(&faulty).unwrap();
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(
            &json!({}),
            &json!({"effects":{"First":null,"Second":null},"receipts":[],"replies":[]}),
        )
        .unwrap();
    session
        .replay(&json!({"kind":"deliver","request":"First"}))
        .unwrap();
    session
        .replay(&json!({"kind":"loseResponse","request":"First"}))
        .unwrap();
    session
        .replay(&json!({"kind":"deliver","request":"Second"}))
        .unwrap();
    assert_eq!(
        session.dispatch(&json!({"kind":"deliver","request":"First"}), |_| panic!(
            "duplicate delivery reached the domain"
        )),
        Err(ValidationError::OutsideModel(
            "transition RetryPreservesEffect".to_owned()
        )),
    );
}

#[test]
fn crash_recovery_safety_and_progress_are_separate_questions() {
    let source = include_str!("../../../examples/model-patterns/crash-recovery/transaction.model");
    let model = rmodel::parse(source).unwrap();
    for (question, expected) in [
        ("RecoversWhenScheduled", Verdict::Holds),
        ("RecoveryWithoutScheduling", Verdict::Violated),
    ] {
        let result = rcheck::check(&model, question, &Selection::All, Budget::default()).unwrap();
        assert_eq!(result.answers.len(), 2);
        assert!(
            result
                .answers
                .iter()
                .all(|answer| answer.verdict == expected),
            "{result:?}"
        );
    }

    let fault = source.replace("phase = Prepared }", "phase = Prepared ledger = Posted }");
    for (source, expected, depth) in [
        (source, Verdict::Holds, None),
        (fault.as_str(), Verdict::Violated, Some(4)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let result = rcheck::check(
            &model,
            "RestartIsAtomic",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_atom_transition_answers(&result, expected, depth);
    }
}

#[test]
fn instance_fairness_distinguishes_starvation_from_fifo_progress() {
    for (source, checks) in [
        (
            include_str!("../../../examples/model-patterns/worker-fairness/competitive.model"),
            &[
                ("GroupStrong", Verdict::Violated),
                ("InstanceWeak", Verdict::Violated),
                ("InstanceStrong", Verdict::Holds),
            ][..],
        ),
        (
            include_str!("../../../examples/model-patterns/worker-fairness/fifo.model"),
            &[("FifoWeak", Verdict::Holds)][..],
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        let json = serde_json::to_string(&rcheck::ir::to_ir(&model).unwrap()).unwrap();
        let (model, omitted) = rcheck::ir::parse(&json).unwrap();
        assert!(omitted.is_empty());
        for (question, expected) in checks {
            let report =
                rcheck::check(&model, question, &Selection::All, Budget::default()).unwrap();
            assert_eq!(report.answers.len(), 2);
            assert!(
                report
                    .answers
                    .iter()
                    .all(|answer| &answer.verdict == expected),
                "{report:?}"
            );
        }
    }
}

#[test]
fn a_stale_tab_cannot_change_the_server_setting() {
    let source = include_str!("../../../examples/model-patterns/tab-settings/settings.model");
    let fault = source.replace(
        "if { tab.loaded = version } then {
      value = setting",
        "value = setting
    if { tab.loaded = version } then {",
    );
    for (source, expected, depth) in [
        (source, Verdict::Holds, None),
        (fault.as_str(), Verdict::Violated, Some(4)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "StaleSaveKeepsServer",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers.len(), 3);
        for answer in report.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (expected.clone(), depth),
                "{:?}",
                answer.detail
            );
        }
    }
}

#[test]
fn a_cancelled_waiter_cannot_be_reaccepted_when_the_buffer_drains() {
    let source = include_str!("../../../examples/model-patterns/backpressure/channel.model");
    let faulty = source.replace(
        "then removeAll(waiting, item) else waiting",
        "then waiting else waiting",
    );
    for (source, expected, depth) in [
        (source, Verdict::Holds, None),
        (faulty.as_str(), Verdict::Violated, Some(3)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "CancellationStays",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers.len(), 3);
        for answer in report.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (expected.clone(), depth),
                "{:?}",
                answer.detail
            );
        }
    }
}

#[test]
fn stream_terminal_distinguishes_draining_and_failure() {
    let source = include_str!("../../../examples/model-patterns/stream-terminal/stream.model");
    let drop_pending = source.replace(
        "then Closed else Closing }",
        "then Closed else Closed pending = Chunks[] }",
    );
    let end_on_error = source.replace(
        "terminal = if { phase = Closed } then End else ReadError",
        "terminal = End",
    );
    for (source, question, expected, depth) in [
        (source, "ClosingPreservesPending", Verdict::Holds, None),
        (source, "FailureRejectsPending", Verdict::Holds, None),
        (source, "ErrorIsNotEnd", Verdict::Holds, None),
        (
            &drop_pending,
            "ClosingPreservesPending",
            Verdict::Violated,
            Some(2),
        ),
        (&end_on_error, "ErrorIsNotEnd", Verdict::Violated, Some(2)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(&model, question, &Selection::All, Budget::default()).unwrap();
        assert_eq!(report.answers.len(), 3);
        for answer in report.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (expected.clone(), depth),
                "{:?}",
                answer.detail
            );
        }
    }
}

#[test]
fn dispatch_cannot_transfer_an_event_to_a_new_subscription() {
    let source = include_str!("../../../examples/model-patterns/subscriptions/notification.model");
    let faulty = source.replace("ticket & Subscriber.active", "ticket.owners.active");
    for (source, expected, depth) in [
        (source, Verdict::Holds, None),
        (faulty.as_str(), Verdict::Violated, Some(5)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "DispatchUsesSameRegistration",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_atom_transition_answers(&report, expected, depth);
    }
}

#[test]
fn a_fact_that_fixes_a_relation_does_not_require_enumerating_it() {
    let model = rmodel::parse(
        "model FixedOwners
enum Account = A | B | C | D | E
enum Role = X | Y | Z | W
rel owners: Account -> set Role
var seen: lone Role
fact { { A -> X + B -> Y = owners } }
init { no seen }
behavior observe() { update { seen = A.owners } }
invariant OwnersStay { owners = A -> X + B -> Y }
reachable SeesX { seen = X }
scope steps = 4",
    )
    .unwrap();
    let budget = Budget {
        states: 10_000,
        work: 1_000_000,
        ..Budget::default()
    };
    for (name, expected, depth) in [
        ("OwnersStay", Verdict::Holds, None),
        ("SeesX", Verdict::Reachable, Some(1)),
    ] {
        let result =
            rcheck::check(&model, name, &Selection::Named("explicit".into()), budget).unwrap();
        let answer = &result.answers[0];
        assert_eq!(
            (answer.verdict.clone(), answer.depth),
            (expected, depth),
            "{result:?}"
        );
    }
}

#[test]
fn fixed_relation_lowering_keeps_domain_constraints_and_other_facts() {
    for (source, questions) in [
        (
            "model Alternatives
             enum Choice = A | B
             rel selected: one Choice
             fact { selected = A or selected = B }
             reachable FirstChoice { selected = A }
             reachable SecondChoice { selected = B }
             scope steps = 2",
            &[
                ("FirstChoice", Verdict::Reachable),
                ("SecondChoice", Verdict::Reachable),
            ][..],
        ),
        (
            "model Conflict
             enum Choice = A | B
             rel selected: one Choice
             fact { selected = A selected = B }
             reachable FirstChoice { selected = A }
             scope steps = 2",
            &[("FirstChoice", Verdict::Empty)][..],
        ),
        (
            "model Outside
             enum Allowed = A
             enum Disallowed = B
             type Choice = Allowed + Disallowed
             rel selected: one Allowed
             fact { selected = B }
             reachable FirstChoice { selected = A }
             scope steps = 2",
            &[("FirstChoice", Verdict::Empty)][..],
        ),
        (
            "model OutsideNat
             rel selected: one Nat
             fact { selected = -1 }
             reachable Positive { selected = 1 }
             scope Int = -1..1, steps = 2",
            &[("Positive", Verdict::Empty)][..],
        ),
        (
            "model DependsOnAnother
             enum Choice = A | B
             rel selected: one Choice
             rel copy: one Choice
             fact { copy = selected selected = A }
             reachable FirstChoice { copy = A }
             scope steps = 2",
            &[("FirstChoice", Verdict::Reachable)][..],
        ),
    ] {
        let model = rmodel::parse(source).unwrap();
        for (question, expected) in questions {
            let result =
                rcheck::check(&model, question, &Selection::All, Budget::default()).unwrap();
            let complete: Vec<_> = result
                .answers
                .iter()
                .filter(|answer| answer.coverage == rcheck::Coverage::Complete)
                .collect();
            assert_eq!(complete.len(), 3, "{result:?}");
            for answer in complete {
                assert_eq!(answer.verdict, *expected, "{result:?}");
            }
        }
    }
}

#[test]
fn repeated_loss_breaks_refund_progress_under_weak_confirmation_fairness() {
    let source = include_str!("../../../examples/model-patterns/compensation/saga.model");
    let weak = source.replace("strong confirmRefund", "weak confirmRefund");
    for (source, expected) in [(source, Verdict::Holds), (&weak, Verdict::Violated)] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(
            &model,
            "RefundEventuallyConfirmed",
            &Selection::All,
            Budget::default(),
        )
        .unwrap();
        assert_eq!(report.answers.len(), 2, "{report:?}");
        assert!(
            report
                .answers
                .iter()
                .all(|answer| answer.verdict == expected),
            "{report:?}"
        );
        let answer = &report.answers[0];
        assert_eq!(answer.verdict, expected, "{report:?}");
        assert_eq!(answer.coverage, rcheck::Coverage::Complete);
        if expected == Verdict::Violated {
            let lasso = answer
                .lasso
                .as_ref()
                .expect("a retry loop permitted by weak fairness");
            let start = lasso["loopStart"].as_u64().unwrap() as usize;
            let trace = lasso["trace"].as_array().unwrap();
            let actions: Vec<_> = trace[start + 1..]
                .iter()
                .map(|step| step["action"].as_str().unwrap())
                .chain(lasso["loopAction"].as_str())
                .collect();
            assert!(actions.contains(&"a_refund"), "{lasso}");
            assert!(actions.contains(&"a_loseReply"), "{lasso}");
            assert!(!actions.contains(&"a_confirmRefund"), "{lasso}");
        } else {
            assert!(answer.lasso.is_none());
        }
    }
}

#[test]
fn tagged_compare_exchange_detects_value_only_and_wrapping_tags() {
    let normal = include_str!("../../../examples/model-patterns/aba/register.model");
    let value_only = normal.replace("saved = current } then", "saved.slot = current.slot } then");
    let wrapping = normal.replace(
        "tag: current.tag + 1",
        "tag: if { current.tag = 1 } then 0 else current.tag + 1",
    );
    let limits = Budget {
        states: 20_000,
        work: 400_000,
        ..Budget::default()
    };
    for (source, expected, depth) in [
        (normal, Verdict::Holds, None),
        (value_only.as_str(), Verdict::Violated, Some(4)),
        (wrapping.as_str(), Verdict::Violated, Some(4)),
    ] {
        let model = rmodel::parse(source).unwrap();
        let report = rcheck::check(&model, "NoStaleSuccess", &Selection::All, limits).unwrap();
        assert_eq!(report.answers.len(), 3, "{report:?}");
        assert!(!report.disagreement, "{report:?}");
        for answer in report.answers {
            assert_eq!(
                (answer.verdict, answer.depth),
                (expected.clone(), depth),
                "{:?}",
                answer.detail
            );
        }
    }
}

#[test]
fn direct_initial_bindings_preserve_alternatives_and_reject_impossible_initial_states() {
    let budget = Budget {
        states: 200,
        work: 10_000,
        ..Budget::default()
    };
    for (predicate, questions) in [
        (
            "0 = n no saved",
            &[
                ("Zero", Verdict::Reachable, Some(0)),
                ("One", Verdict::Reachable, Some(1)),
            ][..],
        ),
        (
            "n = 0 or n = 1",
            &[
                ("Zero", Verdict::Reachable, Some(0)),
                ("One", Verdict::Reachable, Some(0)),
            ][..],
        ),
        ("no n", &[("Zero", Verdict::Empty, None)][..]),
        ("n = 0 n = 1", &[("Zero", Verdict::Empty, None)][..]),
        ("n = -1", &[("Zero", Verdict::Empty, None)][..]),
    ] {
        let model = rmodel::parse(&format!(
            "model InitialBindings
             var n: one Nat
             var saved: lone Nat
             init {{ {predicate} }}
             behavior increment() {{ guard {{ n = 0 }} update {{ n = n + 1 }} }}
             reachable Zero {{ n = 0 }}
             reachable One {{ n = 1 }}
             scope Int = -1..1, steps = 2",
        ))
        .unwrap();
        for (question, expected, depth) in questions {
            let result = rcheck::check(&model, question, &Selection::All, budget).unwrap();
            assert_eq!(result.answers.len(), 3, "{result:?}");
            for answer in result.answers {
                assert_eq!(
                    (&answer.verdict, answer.depth),
                    (expected, *depth),
                    "{answer:?}"
                );
            }
        }
    }
}
