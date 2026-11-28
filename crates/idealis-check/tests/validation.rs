use idealis_check::conform::{ValidationError, Validator};
use serde_json::json;

#[test]
fn a_domain_callback_only_runs_for_a_command_allowed_by_the_model() {
    let model =
        idealis_model::parse(include_str!("../../../fixtures/rcheck/nat-account.model")).unwrap();
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
    let model =
        idealis_model::parse(include_str!("../../../fixtures/rcheck/nat-account.model")).unwrap();
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
    let model =
        idealis_model::parse(include_str!("../../../fixtures/rcheck/nat-account.model")).unwrap();
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
    let model = idealis_model::parse(
        "model Decrement
         record Amount { units: Nat }
         var n: one Amount
         init { n = Amount {units: 0} }
         behavior down() { update { n = Amount {units: n.units - 1} } }
         invariant NonNegative { n.units >= 0 }
         scope Int = -1..1, steps = 3",
    )
    .unwrap();
    let report = idealis_model::reference::analyze(&model, 100);
    assert_eq!(
        report.scope,
        idealis_model::reference::Outcome::Violated { depth: 1 }
    );
    let all = idealis_check::check(
        &model,
        "NonNegative",
        &idealis_check::Selection::All,
        idealis_check::Budget::default(),
    )
    .unwrap();
    assert_eq!(all.answers.len(), 3, "{all:?}");
    for answer in &all.answers {
        assert_eq!(
            answer.verdict,
            idealis_check::Verdict::Unknown,
            "{answer:?}"
        );
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
    let model = idealis_model::parse(
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
fn cli_projects_derivations_at_the_prefix_and_around_a_command() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    let model = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/auth-history/history.ideal");
    let run = |derive, command: Option<serde_json::Value>| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
            .args(["validate", model.to_str().unwrap(), "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut request = json!({
            "config": {},
            "initial": {"history":{"$sequence":["Home"]},"cursor":0,"current":"Home","signedIn":false},
            "prefix":[{"kind":"signIn"},{"kind":"push","destination":"Account"}],
            "derive": derive
        });
        if let Some(command) = command {
            request["command"] = command;
        }
        write!(child.stdin.take().unwrap(), "{request}").unwrap();
        child.wait_with_output().unwrap()
    };
    let sign_out = json!({"kind":"signOut"});
    let approved = run(json!(["screen"]), Some(sign_out.clone()));
    assert!(approved.status.success(), "{approved:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&approved.stdout).unwrap(),
        json!({
            "state": {"history":{"$sequence":["Home","Account"]},"cursor":1,"current":"Account","signedIn":false},
            "derived": {"before":{"screen":[["Account"]]},"after":{"screen":[["SignIn"]]}}
        })
    );
    for names in [json!(["current"]), json!(["screen", "screen"])] {
        assert_eq!(run(names, Some(sign_out.clone())).status.code(), Some(2));
    }
    let snapshot = run(json!(["screen"]), None);
    assert!(snapshot.status.success(), "{snapshot:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&snapshot.stdout).unwrap(),
        json!({
            "state": {"history":{"$sequence":["Home","Account"]},"cursor":1,"current":"Account","signedIn":true},
            "derived": {"screen":[["Account"]]}
        })
    );
    assert_eq!(
        run(json!(["screen"]), Some(json!(null))).status.code(),
        Some(2)
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
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
    use idealis_check::ir::{E, from_ir, to_ir};

    let source = include_str!("../../../fixtures/rcheck/nat-account.model");
    let model = idealis_model::parse(source).unwrap();
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

#[test]
fn validates_the_saved_queue_witness_without_enumerating_checkpoints() {
    let source =
        include_str!("../../../examples/model-patterns/crash-recovery/restored-queue.ideal");
    let model = idealis_model::parse(source).unwrap();
    let witness = idealis_check::conform::witness(&model, "RestoresFullQueue", 9000).unwrap();
    let trace = &witness["trace"];
    let validator = Validator::new(&model).unwrap();
    let mut session = validator
        .start(&trace["config"], &trace["initial"])
        .unwrap();
    assert_eq!(
        session.validate(&trace["commands"][0]).unwrap(),
        trace["finalState"]
    );
    session.replay(&trace["commands"][0]).unwrap();
    assert!(matches!(
        session.validate(&trace["commands"][0]),
        Err(ValidationError::Disabled(_))
    ));

    let mut wrong = trace["initial"].clone();
    wrong["saved"]["pending"]["$sequence"] = json!(["First", "Second", "Third", "Fourth", "First"]);
    assert!(matches!(
        validator.start(&trace["config"], &wrong),
        Err(ValidationError::InvalidInput(_))
    ));
    wrong["saved"] = trace["initial"]["saved"].clone();
    wrong["restored"] = wrong["saved"].clone();
    assert!(matches!(
        validator.start(&trace["config"], &wrong),
        Err(ValidationError::InvalidInput(_))
    ));
}

#[test]
fn validates_supplied_arguments_and_transition_properties_in_a_large_domain() {
    let model = idealis_model::parse(
        "model RestoreArgument
         enum Work = First | Second | Third | Fourth
         sequence Batch = Work, max = 4
         sequence History = Batch, max = 2
         var current: one History
         init { current = History[] }
         behavior replace(history: History) { update { current = history } }
         transition Matches on replace { after(current) = history }
         transition NonEmpty on replace { some at(history, 0) }
         scope Int = 0..2, steps = 2",
    )
    .unwrap();
    assert!(idealis_model::reference::Machine::new(&model).is_err());
    let validator = Validator::new(&model).unwrap();
    let session = validator
        .start(&json!({}), &json!({"current":{"$sequence":[]}}))
        .unwrap();
    let held = json!({"$sequence":[{"$sequence":["Third","First"]}]});
    assert_eq!(
        session
            .validate(&json!({"kind":"replace","history":held}))
            .unwrap(),
        json!({"current":held})
    );
    assert_eq!(
        session.validate(&json!({"kind":"replace","history":{"$sequence":[]}})),
        Err(ValidationError::OutsideModel("transition NonEmpty".into()))
    );
}

#[test]
fn validates_a_supplied_relation_without_enumerating_its_subsets() {
    let model = idealis_model::parse(
        "model GivenLinks
         enum Node = A | B | C | D | E
         rel links: Node -> set Node
         var enabled: one Bool
         fact { no C.links }
         init { not enabled }
         behavior ready() { guard { B in A.links } update { enabled = true } }
         scope steps = 2",
    )
    .unwrap();
    assert!(idealis_model::reference::enumerable(&model).is_err());
    let validator = Validator::new(&model).unwrap();
    let mut config = json!({"links":{"A":["B","A"],"B":[],"C":[],"D":[],"E":[]}});
    let initial = json!({"enabled":false});
    let session = validator.start(&config, &initial).unwrap();
    assert_eq!(
        session.validate(&json!({"kind":"ready"})).unwrap(),
        json!({"enabled":true})
    );
    config["links"]["C"] = json!(["A"]);
    assert!(matches!(
        validator.start(&config, &initial),
        Err(ValidationError::InvalidInput(_))
    ));
    config["links"]["C"] = json!([]);
    config["links"]["A"] = json!(["B", "B"]);
    assert!(matches!(
        validator.start(&config, &initial),
        Err(ValidationError::InvalidInput(_))
    ));
}
