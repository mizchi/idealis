use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn rcheck(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .args(args)
        .output()
        .unwrap()
}

fn coverage(path: &Path, flags: &[&str], exit: i32) -> Value {
    let mut args = vec!["coverage", path.to_str().unwrap()];
    args.extend(flags);
    let output = rcheck(&args);
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn coverage_combines_actions_with_update_arms_and_keeps_ir_limit_visible() {
    let model = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/crash-recovery/receipt-wire.model");
    let report = coverage(&model, &[], 0);
    assert_eq!(report["coverage"], "complete");
    assert_eq!(report["branchCoverage"], "complete");
    let actions = report["actions"].as_array().unwrap();
    assert!(actions.iter().all(|action| action["status"] == "reachable"));
    let deliver = actions
        .iter()
        .find(|action| action["name"] == "deliver")
        .unwrap();
    let incompatible = deliver["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"].as_str().unwrap().ends_with(":59:7"))
        .unwrap();
    assert_eq!(incompatible["then"]["verdict"], "unreachable");
    assert_eq!(incompatible["then"]["expectation"], "matched");
    assert_eq!(incompatible["else"]["verdict"], "reachable");

    let file = std::env::temp_dir().join(format!("rcheck-coverage-{}.json", std::process::id()));
    let ir = rcheck(&["generate", model.to_str().unwrap(), "ir"]);
    assert!(ir.status.success(), "{ir:?}");
    std::fs::write(&file, ir.stdout).unwrap();
    let decoded = coverage(&file, &[], 3);
    std::fs::remove_file(file).unwrap();
    assert_eq!(decoded["coverage"], "complete");
    assert_eq!(decoded["branchCoverage"], "unsupported");
    assert_eq!(decoded["actions"].as_array().unwrap().len(), actions.len());
    assert!(
        decoded["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|action| { action["status"] == "reachable" && action["branches"].is_null() })
    );

    let limited = coverage(&model, &["--max-states", "1"], 3);
    assert_eq!(limited["coverage"], "unknown");
    assert_eq!(limited["branchCoverage"], "unknown");
    assert_eq!(limited["diagnostics"][0]["resource"], "states");
}

#[test]
fn coverage_of_a_project_includes_public_and_internal_operations() {
    let model = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/offline-merge/two-stores/project.rmodel.json");
    let report = coverage(&model, &[], 0);
    let actions = report["actions"].as_array().unwrap();
    assert!(actions.iter().any(|action| action["name"] == "east_commit"));
    assert!(
        actions
            .iter()
            .any(|action| action["name"] == "westDelivery_send")
    );
}

#[test]
fn a_portable_branch_map_recovers_linked_source_diagnostics() {
    let root = std::env::temp_dir().join(format!("rcheck-branch-map-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let host = root.join("host.model");
    let part = root.join("subscriber.model");
    let project = root.join("subscriber.rmodel.json");
    let ir = root.join("subscriber.json");
    let map = root.join("branches.json");
    std::fs::write(&host, "model Subscriber\nscope steps = 3\n").unwrap();
    std::fs::write(&part, "// 受信\nvar @ready: one Bool\ninit { not @ready }\nbehavior @poll() { update {\n  if { @ready } then { @ready = false }\n} }\n").unwrap();
    std::fs::write(
        &project,
        r#"{"fragments":["host.model",{"path":"subscriber.model","namespace":"sub"}]}"#,
    )
    .unwrap();
    let current = coverage(&project, &[], 1);
    let action = "sub_poll";
    let row = &current["actions"][0]["branches"][0];
    assert!(
        row["source"]
            .as_str()
            .unwrap()
            .ends_with("subscriber.model:5:3")
    );
    assert_eq!(row["then"]["verdict"], "unreachable");
    let generate = |target| {
        let output = rcheck(&["generate", project.to_str().unwrap(), target]);
        assert!(output.status.success(), "{output:?}");
        output.stdout
    };
    let model_ir: Value = serde_json::from_slice(&generate("ir")).unwrap();
    std::fs::write(&ir, model_ir.to_string()).unwrap();
    let branch_map = generate("branch-map");
    std::fs::write(&map, &branch_map).unwrap();
    for source in [host, part, project] {
        std::fs::remove_file(source).unwrap();
    }
    let flags = ["--branch-map", map.to_str().unwrap()];
    assert_eq!(coverage(&ir, &flags, 1), current);
    let output = rcheck(&["branches", ir.to_str().unwrap(), action, flags[0], flags[1]]);
    assert!(output.status.success(), "{output:?}");
    let branches: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(branches["branches"], current["actions"][0]["branches"]);

    let mut changed = model_ir.clone();
    changed["name"] = Value::from("Changed");
    std::fs::write(&ir, changed.to_string()).unwrap();
    let output = rcheck(&["coverage", ir.to_str().unwrap(), flags[0], flags[1]]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("branch map does not match the IR"));

    std::fs::write(&ir, model_ir.to_string()).unwrap();
    let mut damaged: Value = serde_json::from_slice(&branch_map).unwrap();
    damaged["groups"] = serde_json::json!({});
    std::fs::write(&map, damaged.to_string()).unwrap();
    let output = rcheck(&["coverage", ir.to_str().unwrap(), flags[0], flags[1]]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("branch map content does not match"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn coverage_keeps_missing_initial_states_and_broken_declarations_inconclusive() {
    let path = std::env::temp_dir().join(format!("rcheck-coverage-{}.model", std::process::id()));
    for (init, update, exit, scope, status) in [
        ("false", "flag = false", 3, "empty", "empty"),
        ("flag", "flag = none", 3, "unknown", "unknown"),
        ("flag", "flag = false", 1, "complete", "unreachable"),
    ] {
        std::fs::write(
            &path,
            format!("model Flag\nvar flag: one Bool\ninit {{ {init} }}\nbehavior tick() {{ update {{ {update} }} }}\nbehavior blocked() {{ guard {{ false }} }}\nscope steps = 2"),
        )
        .unwrap();
        let report = coverage(&path, &[], exit);
        assert_eq!(report["coverage"], scope);
        assert_eq!(report["actions"][1]["status"], status);
        if scope == "unknown" {
            assert_eq!(report["diagnostics"][0]["kind"], "declaration_violation");
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn a_documented_impossible_arm_is_checked_and_catches_a_newly_reachable_branch() {
    let root = std::env::temp_dir().join(format!("rcheck-expected-arm-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/crash-recovery/receipt-wire.model");
    let annotation = r#" expect unreachable "Compatible encodings decode to the same decision""#;
    let annotated = std::fs::read_to_string(base).unwrap();
    let source = annotated.replace(annotation, "");
    assert_ne!(source, annotated);
    let model = root.join("receipt.model");
    let ir = root.join("receipt.json");
    let map = root.join("branch-map.json");
    let generate = |target| {
        let output = rcheck(&["generate", model.to_str().unwrap(), target]);
        assert!(output.status.success(), "{output:?}");
        output.stdout
    };
    std::fs::write(&model, &source).unwrap();
    let plain_ir = generate("ir");
    std::fs::write(&model, &annotated).unwrap();
    assert_eq!(generate("ir"), plain_ir);
    let normal = coverage(&model, &[], 0);
    let arm = |report: &Value| {
        report["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == "deliver")
            .unwrap()["branches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["then"]["reason"] == "Compatible encodings decode to the same decision")
            .unwrap()["then"]
            .clone()
    };
    assert_eq!(arm(&normal)["verdict"], "unreachable");
    assert_eq!(arm(&normal)["expectation"], "matched");
    assert_eq!(
        arm(&normal)["reason"],
        "Compatible encodings decode to the same decision"
    );
    std::fs::write(&ir, plain_ir).unwrap();
    std::fs::write(&map, generate("branch-map")).unwrap();
    assert_eq!(
        coverage(&ir, &["--branch-map", map.to_str().unwrap()], 0),
        normal
    );
    let low_budget = coverage(&model, &["--max-states", "1"], 3);
    assert_eq!(arm(&low_budget)["expectation"], "unknown");

    let wrong = annotated.replace("accepted != decision", "accepted = decision");
    std::fs::write(&model, wrong).unwrap();
    let mismatch = coverage(&model, &[], 1);
    assert_eq!(mismatch["status"], "expectation_mismatch");
    let actual = arm(&mismatch);
    assert_eq!(actual["verdict"], "reachable");
    assert_eq!(actual["expectation"], "mismatch");
    assert_eq!(
        actual["trace"]["commands"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["kind"],
        "deliver"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_explicit_empty_else_arm_can_be_expected_unreachable() {
    let path =
        std::env::temp_dir().join(format!("rcheck-expected-else-{}.model", std::process::id()));
    let source = r#"model Ready
var ready: one Bool
init { ready }
behavior refresh() {
  update {
    if { ready } then { ready = true }
    else expect unreachable "Refresh preserves readiness" {}
  }
}
scope steps = 2
"#;
    std::fs::write(&path, source).unwrap();
    let reachable = coverage(&path, &[], 0);
    let arm = &reachable["actions"][0]["branches"][0]["else"];
    assert_eq!(arm["expectation"], "matched");
    assert_eq!(arm["verdict"], "unreachable");

    std::fs::write(&path, source.replace("init { ready }", "init { false }")).unwrap();
    let empty = coverage(&path, &[], 3);
    assert_eq!(
        empty["actions"][0]["branches"][0]["else"]["expectation"],
        "unknown"
    );

    std::fs::write(&path, source.replace("Refresh preserves readiness", "")).unwrap();
    let invalid = rcheck(&["coverage", path.to_str().unwrap()]);
    assert_eq!(invalid.status.code(), Some(2), "{invalid:?}");
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("nonempty quoted reason"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn the_same_search_reports_argument_bindings_and_update_arms() {
    let path =
        std::env::temp_dir().join(format!("rcheck-all-arguments-{}.model", std::process::id()));
    std::fs::write(
        &path,
        "model AllowedCalls
        enum Registration = Original | Replacement | Secondary
        var sent: set Registration
        init { no sent }
        behavior sendLegacy(registration: Registration) {
          guard { registration != Replacement registration not in sent }
          update {
            if { no sent } then { sent = registration }
            else { sent = sent + registration }
          }
        }
        scope steps = 4",
    )
    .unwrap();
    let report = coverage(&path, &["--instances"], 1);
    assert_eq!(report["instanceCoverage"], "complete");
    assert_eq!(report["branchCoverage"], "complete");
    let operations = report["actions"].as_array().unwrap();
    let send = operations
        .iter()
        .find(|action| action["name"] == "sendLegacy")
        .unwrap();
    assert_eq!(send["status"], "reachable");
    assert_eq!(send["branches"][0]["then"]["verdict"], "reachable");
    assert_eq!(send["branches"][0]["else"]["verdict"], "reachable");
    let individual = rcheck(&[
        "dead-code",
        path.to_str().unwrap(),
        "--instances",
        "sendLegacy",
    ]);
    assert_eq!(individual.status.code(), Some(1));
    let individual: Value = serde_json::from_slice(&individual.stdout).unwrap();
    assert_eq!(send["instances"], individual["instances"]);
    assert!(send["instances"].as_array().unwrap().iter().any(|row| {
        row["args"]["registration"] == "Replacement" && row["status"] == "unreachable"
    }));
    let limited = coverage(&path, &["--instances", "--max-arguments", "1"], 3);
    assert_eq!(limited["instanceCoverage"], "unknown");
    assert_eq!(limited["diagnostics"][0]["resource"], "arguments");
    assert!(
        limited["actions"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|row| row["instances"].as_array().unwrap())
            .all(|row| row["status"] != "unreachable")
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn an_oversized_catalogue_keeps_smaller_actions_and_is_inconclusive() {
    let directory = std::env::temp_dir().join(format!("rcheck-catalogue-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("test.model");
    let choices = (0..17)
        .map(|index| format!("C{index}"))
        .collect::<Vec<_>>()
        .join(" | ");
    std::fs::write(&model, format!(
        "model ManyCalls\nenum Choice = {choices}\nbehavior send(a: Choice, b: Choice, c: Choice) {{}}\nbehavior heartbeat() {{}}\nscope steps = 2"
    )).unwrap();
    let result = coverage(&model, &["--instances"], 3);
    assert_eq!(result["coverage"], "complete");
    assert_eq!(result["instanceCoverage"], "partial");
    let actions = result["actions"].as_array().unwrap();
    assert_eq!(actions[0]["status"], "reachable");
    assert!(actions[0]["instances"].is_null());
    assert!(
        actions[0]["instanceDiagnostic"]
            .as_str()
            .unwrap()
            .contains("instance limit")
    );
    assert_eq!(actions[1]["instances"][0]["status"], "reachable");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn decoder_legacy_traffic_has_its_own_argument_domain() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/crash-recovery/decoder-membership.model");
    let report = coverage(&path, &["--instances"], 0);
    let actions = report["actions"].as_array().unwrap();
    for name in ["sendLegacy", "takeLegacy"] {
        let action = actions.iter().find(|row| row["name"] == name).unwrap();
        let registrations = action["instances"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["args"]["registration"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(registrations, ["Original", "Secondary"]);
    }
    let take = actions
        .iter()
        .find(|row| row["name"] == "takeLegacy")
        .unwrap();
    assert_eq!(take["branches"][0]["then"]["expectation"], "matched");
    let seal = actions
        .iter()
        .find(|row| row["name"] == "takeSeal")
        .unwrap();
    assert!(seal["instances"].as_array().unwrap().iter().any(|row| {
        row["args"]["registration"] == "Replacement" && row["status"] == "reachable"
    }));
    let model = rmodel::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
    let validator = rcheck::conform::Validator::new(&model).unwrap();
    let initial = serde_json::json!({
        "live": {"Primary":"Original", "Hedge":"Secondary"}, "phase":"Idle",
        "cohort": [], "stopped": [], "sent": [], "taken": [], "sealed": [],
        "promised": [], "decoded": [], "dropped": [], "replacementDecoded":false
    });
    let mut session = validator.start(&serde_json::json!({}), &initial).unwrap();
    for kind in ["sendLegacy", "takeLegacy"] {
        let wrong = serde_json::json!({"kind":kind,"registration":"Replacement"});
        assert!(matches!(
            session.validate(&wrong),
            Err(rcheck::conform::ValidationError::InvalidInput(_))
        ));
    }
    let stop = serde_json::json!({"kind":"stop","registration":"Replacement"});
    assert!(matches!(
        session.validate(&stop),
        Err(rcheck::conform::ValidationError::Disabled(_))
    ));
    session
        .replay(&serde_json::json!({"kind":"begin"}))
        .unwrap();
    session
        .replay(&serde_json::json!({"kind":"replace"}))
        .unwrap();
    assert_eq!(
        session.validate(&stop).unwrap()["stopped"],
        serde_json::json!(["Replacement"])
    );
}

#[test]
fn a_value_arm_expectation_catches_restoring_an_open_checkpoint() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/crash-recovery/receipt-checkpoint.model");
    let root =
        std::env::temp_dir().join(format!("rcheck-value-expectation-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let model = root.join("checkpoint.model");
    let ir = root.join("checkpoint.json");
    let map = root.join("branch-map.json");
    let annotated = std::fs::read_to_string(&source).unwrap();
    let generate = |target| {
        let output = rcheck(&["generate", model.to_str().unwrap(), target]);
        assert!(output.status.success(), "{output:?}");
        output.stdout
    };
    let normal = coverage(&source, &[], 0);
    let value_arm = |report: &Value| {
        report["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|action| action["name"] == "install")
            .unwrap()["branches"]
            .as_array()
            .unwrap()
            .iter()
            .find(|branch| branch["kind"] == "value")
            .unwrap()["else"]
            .clone()
    };
    assert_eq!(value_arm(&normal)["expectation"], "matched");
    assert_eq!(
        value_arm(&normal)["reason"],
        "The required checkpoint is the closed revision"
    );
    let limited = coverage(&source, &["--max-states", "1"], 3);
    assert_eq!(value_arm(&limited)["expectation"], "unknown");

    std::fs::write(
        &model,
        annotated.replace(
            "expect unreachable \"The required checkpoint is the closed revision\" ",
            "",
        ),
    )
    .unwrap();
    let plain_ir = generate("ir");
    std::fs::write(&model, &annotated).unwrap();
    assert_eq!(generate("ir"), plain_ir);
    std::fs::write(&ir, plain_ir).unwrap();
    std::fs::write(&map, generate("branch-map")).unwrap();
    assert_eq!(
        coverage(&model, &[], 0),
        coverage(&ir, &["--branch-map", map.to_str().unwrap()], 0)
    );

    std::fs::write(
        &model,
        annotated.replace("required = AfterClose", "required = BeforeClose"),
    )
    .unwrap();
    let changed = coverage(&model, &[], 1);
    assert_eq!(changed["status"], "expectation_mismatch");
    let arm = value_arm(&changed);
    assert_eq!(arm["expectation"], "mismatch");
    assert_eq!(arm["trace"]["commands"].as_array().unwrap().len(), 3);
    assert_eq!(arm["trace"]["commands"][2]["kind"], "install");
    assert_eq!(arm["trace"]["finalState"]["closed"], false);
    std::fs::remove_dir_all(root).unwrap();
}
