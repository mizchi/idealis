use idealis_check::{
    Answer, Budget, Checker, Coverage, Diagnostic, Question, Selection, Support, Verdict,
    check_with,
};

struct Adapter {
    name: &'static str,
    support: Support,
    result: Option<(Coverage, Verdict)>,
}

impl Checker for Adapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn supports(&self, _: &idealis_model::Model, _: &Question) -> Support {
        self.support.clone()
    }

    fn answer(&self, _: &idealis_model::Model, question: &Question, budget: Budget) -> Answer {
        let (coverage, verdict) = self
            .result
            .clone()
            .expect("unsupported adapter was invoked");
        assert_eq!(budget.states, 2000);
        let diagnostics = if verdict == Verdict::Unknown {
            vec![Diagnostic::Budget {
                resource: "states".into(),
            }]
        } else {
            Vec::new()
        };
        Answer {
            question: question.name.clone(),
            checker: self.name,
            verdict,
            coverage,
            depth: None,
            detail: None,
            diagnostics,
            lasso: None,
        }
    }
}

fn bank() -> idealis_model::Model {
    let legacy = include_str!("../../../fixtures/rcheck/ir-compat/pkl-bank-v1.json");
    idealis_check::ir::parse(legacy).unwrap().0
}

#[test]
fn a_registered_adapter_uses_its_own_bound_and_can_decline_a_question() {
    let model = bank();
    let shallow = Coverage::Bounded { steps: 1 };
    let reference = idealis_check::checkers::Reference;
    let short = Adapter {
        name: "short",
        support: Support::Yes(shallow),
        result: Some((shallow, Verdict::Unreachable)),
    };
    let unavailable = Adapter {
        name: "unsupported",
        support: Support::No("numeric fragment".into()),
        result: None,
    };
    let registered: [&dyn Checker; 3] = [&short, &unavailable, &reference];
    let budget = Budget {
        states: 2000,
        ..Budget::default()
    };
    let report = check_with(&model, "Rich", &Selection::All, budget, &registered).unwrap();
    assert_eq!(report.answers[0].coverage, shallow);
    assert_eq!(report.answers[1].verdict, Verdict::Reachable);
    assert_eq!(report.answers[1].depth, Some(1));
    assert!(!report.disagreement);
    assert_eq!(report.unsupported[0].checker, "unsupported");
    assert_eq!(report.unsupported[0].reason, "numeric fragment");
    let auto = check_with(&model, "Rich", &Selection::Auto, budget, &registered).unwrap();
    assert_eq!(auto.answers[0].checker, "reference");

    let includes_witness = Coverage::Bounded { steps: 2 };
    let contradicts = Adapter {
        name: "contradicts",
        support: Support::Yes(includes_witness),
        result: Some((includes_witness, Verdict::Unreachable)),
    };
    let report = check_with(
        &model,
        "Rich",
        &Selection::All,
        budget,
        &[&contradicts, &reference],
    )
    .unwrap();
    assert!(report.disagreement);
}

#[test]
fn a_registered_adapter_preserves_budget_failure_and_rejects_bad_metadata() {
    let model = bank();
    let budget = Budget {
        states: 2000,
        ..Budget::default()
    };
    let bounded = Coverage::Bounded { steps: 2 };
    for (support, value, expected) in [
        (
            Support::Yes(Coverage::Complete),
            (Coverage::Complete, Verdict::Unknown),
            Verdict::Unknown,
        ),
        (
            Support::Yes(bounded),
            (Coverage::Complete, Verdict::Unreachable),
            Verdict::Error,
        ),
        (
            Support::Yes(Coverage::Complete),
            (Coverage::Complete, Verdict::Holds),
            Verdict::Error,
        ),
    ] {
        let adapter = Adapter {
            name: "custom",
            support,
            result: Some(value),
        };
        let report = check_with(
            &model,
            "Rich",
            &Selection::Named("custom".into()),
            budget,
            &[&adapter],
        )
        .unwrap();
        let answer = &report.answers[0];
        assert_eq!(answer.verdict, expected);
        assert!(!answer.definite());
        if expected == Verdict::Unknown {
            assert_eq!(
                answer.diagnostics,
                vec![Diagnostic::Budget {
                    resource: "states".into()
                }]
            );
        } else {
            assert!(answer.detail.is_some());
        }
        assert!(
            check_with(
                &model,
                "Rich",
                &Selection::All,
                budget,
                &[&adapter, &adapter]
            )
            .unwrap_err()
            .to_string()
            .contains("duplicate checker custom")
        );
    }
}
