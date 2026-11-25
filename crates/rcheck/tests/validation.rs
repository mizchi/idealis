use rcheck::conform::{ValidationError, Validator};
use serde_json::json;

#[test]
fn a_domain_callback_only_runs_for_a_command_allowed_by_the_model() {
    let model = rmodel::parse(include_str!("../../../fixtures/rcheck/nat-account.model")).unwrap();
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(&json!({"limit": 3}), &json!({"balance":{"units":3}}))
        .unwrap();

    for (units, expected) in [
        (-1, "invalid"),
        (6, "invalid"),
        (4, "disabled"),
        (0, "disabled"),
    ] {
        let error = session
            .dispatch(&json!({"kind":"withdraw","amount":{"units":units}}), |_| {
                panic!("domain ran for {units}")
            })
            .unwrap_err();
        match (expected, error) {
            ("invalid", ValidationError::InvalidInput(_))
            | ("disabled", ValidationError::Disabled(_)) => {}
            (_, other) => panic!("{units}: {other:?}"),
        }
    }

    let request = json!({"kind":"withdraw","amount":{"units":2}});
    let state = json!({"balance":{"units":1}});
    assert_eq!(session.validate(&request).unwrap(), state);
    assert_eq!(
        session.dispatch(&request, |actual| {
            assert_eq!(actual, &request);
            Ok((true, state.clone()))
        }),
        Ok(state)
    );
    assert!(matches!(
        session.validate(&request),
        Err(ValidationError::Disabled(_))
    ));
}

#[test]
fn facts_and_initial_state_are_checked_before_domain_processing() {
    let model = rmodel::parse(include_str!("../../../fixtures/rcheck/nat-account.model")).unwrap();
    let validator = Validator::new(&model).unwrap();
    assert!(matches!(
        validator.start(&json!({"limit":4}), &json!({"balance":{"units":3}})),
        Err(ValidationError::InvalidInput(_))
    ));
    assert!(matches!(
        validator.start(&json!({"limit":3}), &json!({"balance":{"units":2}})),
        Err(ValidationError::InvalidInput(_))
    ));
}

#[test]
fn a_diverging_domain_instance_cannot_be_reused() {
    let model = rmodel::parse(include_str!("../../../fixtures/rcheck/nat-account.model")).unwrap();
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(&json!({"limit":3}), &json!({"balance":{"units":3}}))
        .unwrap();
    let command = json!({"kind":"withdraw","amount":{"units":1}});
    assert!(matches!(
        session.dispatch(&command, |_| Ok((true, json!({"balance":{"units":3}})))),
        Err(ValidationError::Domain(_))
    ));
    assert!(matches!(
        session.dispatch(&command, |_| panic!("unverified state")),
        Err(ValidationError::Domain(_))
    ));
}

#[test]
fn a_negative_result_in_a_nat_field_leaves_scope_in_both_checkers() {
    let model = rmodel::parse(
        "model Decrement
         record Amount { units: Nat }
         var n: one Amount
         init { n = Amount {units: 0} }
         behavior down() { update { n = Amount {units: n.units - 1} } }
         invariant NonNegative { n.units >= 0 }
         scope Int = -1..1, steps = 3",
    )
    .unwrap();
    let report = rmodel::reference::analyze(&model, 100);
    assert_eq!(
        report.scope,
        rmodel::reference::Outcome::Violated { depth: 1 }
    );
    let all = rcheck::check(
        &model,
        "NonNegative",
        &rcheck::Selection::All,
        rcheck::Budget::default(),
    )
    .unwrap();
    assert_eq!(all.answers.len(), 2, "{all:?}");
    for answer in &all.answers {
        assert_eq!(answer.verdict, rcheck::Verdict::Unknown, "{answer:?}");
    }
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(&json!({}), &json!({"n":{"units":0}}))
        .unwrap();
    assert!(matches!(
        session.dispatch(&json!({"kind":"down"}), |_| panic!(
            "negative Nat reached domain"
        )),
        Err(ValidationError::OutsideModel(_))
    ));
}

#[test]
fn a_next_state_must_satisfy_the_model_invariants() {
    let model = rmodel::parse(
        "model Lock
         enum Status = Open | Locked
         var value: one Status
         init { value = Locked }
         behavior unlock() { update { value = Open } }
         invariant AlwaysLocked { value = Locked }
         scope steps = 2",
    )
    .unwrap();
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(&json!({}), &json!({"value":"Locked"}))
        .unwrap();
    assert_eq!(
        session.dispatch(&json!({"kind":"unlock"}), |_| panic!(
            "violated invariant reached domain"
        )),
        Err(ValidationError::OutsideModel(
            "invariant AlwaysLocked".into()
        ))
    );
}

#[test]
fn cli_validates_an_accepted_prefix_without_running_the_domain() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rcheck/nat-account.model");
    let run = |units| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .arg("validate")
            .arg(&model)
            .arg("-")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let request = json!({
            "config":{"limit":3},
            "initial":{"balance":{"units":3}},
            "prefix":[{"kind":"withdraw","amount":{"units":2}}],
            "command":{"kind":"withdraw","amount":{"units":units}}
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(request.to_string().as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let approved = run(1);
    assert!(approved.status.success(), "{approved:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&approved.stdout).unwrap(),
        json!({"state":{"balance":{"units":0}}})
    );
    let denied = run(2);
    assert_eq!(denied.status.code(), Some(2));
    assert!(
        String::from_utf8(denied.stderr)
            .unwrap()
            .contains("guard is false")
    );
}

#[test]
fn nat_round_trips_and_ir_passes_through_the_same_arithmetic_checks() {
    use rcheck::ir::{E, from_ir, to_ir};

    let source = include_str!("../../../fixtures/rcheck/nat-account.model");
    let model = rmodel::parse(source).unwrap();
    let mut ir = to_ir(&model).unwrap();
    assert_eq!(to_ir(&from_ir(&ir).unwrap()).unwrap(), ir);

    let E::Record { fields, .. } = &mut ir.actions[0].assignments[0].value else {
        panic!("withdrawal assigns a record");
    };
    fields[0] = E::Add {
        left: Box::new(E::Literal { value: i64::MAX }),
        right: Box::new(E::Literal { value: 1 }),
    };
    let error = from_ir(&ir).unwrap_err();
    assert!(error.contains("integer arithmetic"), "{error}");
}
