use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn project(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rcheck/linked")
        .join(name)
        .join("project.rmodel.json")
}

fn invoke(args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn both_linked_projects_are_checked_through_the_same_cli() {
    for (name, checks) in [
        (
            "withdrawal",
            vec![
                ("RejectionPreservesBalance", "holds"),
                ("SuccessDebitsAmount", "holds"),
                ("CanWithdrawExactBalance", "reachable"),
                ("CanReject", "reachable"),
            ],
        ),
        (
            "raft",
            vec![
                ("ElectionSafety", "holds"),
                ("SomeLeader", "reachable"),
                ("SplitVote", "reachable"),
            ],
        ),
    ] {
        let path = project(name);
        let reports = invoke(&[path.to_str().unwrap(), "--checker", "reference"]);
        for (check, expected) in checks {
            let answer = &reports
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["question"] == check)
                .unwrap()["answers"][0];
            assert_eq!(answer["verdict"], expected, "{name}: {check}: {answer}");
            assert_eq!(answer["coverage"]["kind"], "complete");
        }
        let ir = invoke(&["generate", path.to_str().unwrap(), "ir"]);
        assert!(!ir["actions"].as_array().unwrap().is_empty(), "{name}");
    }
}

#[test]
fn boundary_cases_show_both_sides_of_the_business_and_election_thresholds() {
    let pair = |name, kind, action| {
        let model = project(name);
        let response = invoke(&["generate", model.to_str().unwrap(), "pbt-cases"]);
        response["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|pair| pair["kind"] == kind && pair["action"] == action)
            .unwrap()
            .clone()
    };

    let request = pair("withdrawal", "integer", "requestWithdrawal");
    for (side, amount) in [(0, 0), (1, 1)] {
        let example = &request["examples"][side];
        assert_eq!(example["command"]["amount"]["units"], amount);
        assert_eq!(example["accepted"], side == 1);
    }

    let reject = pair("withdrawal", "continuation", "rejectWithdrawal");
    let exact = &reject["examples"][0];
    let exceeds = &reject["examples"][1];
    assert_eq!(exact["prefix"][0]["amount"]["units"], 2);
    assert_eq!(exceeds["prefix"][0]["amount"]["units"], 3);
    assert_eq!(exact["accepted"], false);
    assert_eq!(exceeds["accepted"], true);
    assert_eq!(exceeds["state"]["result"], "InsufficientFunds");
    assert_eq!(exceeds["state"]["balance"]["units"], 2);

    let election = pair("raft", "transition", "elect");
    let before = &election["examples"][0];
    let after = &election["examples"][1];
    assert_eq!(before["accepted"], false);
    assert_eq!(after["accepted"], true);
    assert_eq!(before["command"], after["command"]);
    assert_eq!(
        after["prefix"].as_array().unwrap().last().unwrap()["kind"],
        "grantVote"
    );
}

#[test]
fn the_election_check_catches_double_voting() {
    let source = rcheck::project::load_source(project("raft").to_str().unwrap()).unwrap();
    let faulty = source.replace("v != c no v.votes", "v != c");
    assert_ne!(source, faulty);
    let model = rmodel::parse(&faulty).unwrap();
    let report = rmodel::reference::analyze(&model, rcheck::Budget::default().states);
    assert_eq!(report.scope, rmodel::reference::Outcome::Holds);
    let verdict = report
        .verdicts
        .iter()
        .find(|v| v.check == "ElectionSafety")
        .unwrap();
    assert!(matches!(
        verdict.outcome,
        rmodel::reference::Outcome::Violated { .. }
    ));
}

#[test]
fn boundary_generation_fails_when_exploration_is_incomplete() {
    let source = rcheck::project::load_source(project("withdrawal").to_str().unwrap()).unwrap();
    let model = rmodel::parse(&source).unwrap();
    assert!(
        rcheck::conform::boundary_cases(&model, 1)
            .unwrap_err()
            .contains("complete, nonempty")
    );
}

#[test]
fn withdrawal_bindings_and_property_stay_in_sync() {
    let path = project("withdrawal");
    let source = rcheck::project::load_source(path.to_str().unwrap()).unwrap();
    let model = rmodel::parse(&source).unwrap();
    assert_eq!(
        include_str!("generated/withdrawal.rs"),
        rmodel::bindings::rust(&model, &source)
    );
    assert_eq!(
        include_str!("../../../tests/rcheck/generated/withdrawal.ts"),
        rmodel::bindings::typescript(&model)
    );
    assert_eq!(
        include_str!("../../../tests/rcheck/generated/withdrawal-boundary.ts"),
        rcheck::conform::boundary_typescript(&model, rcheck::Budget::default().states).unwrap()
    );
}

#[test]
fn raw_boundary_cases_preserve_large_integers_but_typescript_refuses_rounding() {
    let model = rmodel::parse(
        "model Large
         behavior positive(value: Int) { guard { value > 9007199254740992 } }
         scope Int = 9007199254740992..9007199254740993, steps = 1",
    )
    .unwrap();
    let budget = rcheck::Budget::default().states;
    let raw = rcheck::conform::boundary_cases(&model, budget).unwrap();
    assert_eq!(
        raw["pairs"][0]["examples"][1]["command"]["value"].as_i64(),
        Some(9_007_199_254_740_993)
    );
    assert!(
        rcheck::conform::boundary_typescript(&model, budget)
            .unwrap_err()
            .contains("safe integers")
    );
}

#[test]
fn go_boundary_generation_uses_the_linked_model_and_config_type() {
    let path = project("withdrawal");
    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .args(["generate", path.to_str().unwrap(), "pbt-go"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        text,
        include_str!("../../../tests/rcheck/go/withdrawal/boundary_generated_test.go")
    );
    let source = rcheck::project::load_source(path.to_str().unwrap()).unwrap();
    let model = rmodel::parse(&source).unwrap();
    assert_eq!(
        rmodel::bindings::go(&model, "withdrawal"),
        include_str!("../../../tests/rcheck/go/withdrawal/bindings.go")
    );
    let mut literal = text.split('`');
    let embedded: Value = serde_json::from_str(literal.nth(1).unwrap()).unwrap();
    assert_eq!(
        embedded,
        invoke(&["generate", path.to_str().unwrap(), "pbt-cases"])
    );
    assert!(text.contains("CheckBoundaryCases(f *testing.F, create func(Config) Withdrawal)"));
}

#[test]
fn explicit_checker_finishes_negative_checks_for_values_within_scope() {
    let withdrawal = project("withdrawal");
    let row = invoke(&[
        withdrawal.to_str().unwrap(),
        "--checker",
        "explicit",
        "--question",
        "RejectionPreservesBalance",
    ]);
    assert_eq!(row[0]["answers"][0]["verdict"], "holds");
    assert_eq!(row[0]["answers"][0]["coverage"]["kind"], "complete");
}

#[test]
fn help_lists_existing_entry_points_without_opening_a_model() {
    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    assert!(help.lines().any(|line| line.contains("rcheck witness")));
    assert!(help.lines().any(|line| line.contains("rcheck generate")));
}

#[test]
fn syntax_and_type_errors_point_to_the_original_fragment() {
    let root =
        std::env::temp_dir().join(format!("rcheck-fragment-location-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let manifest = root.join("project.rmodel.json");
    let checks = root.join("checks.model");
    std::fs::write(
        &manifest,
        r#"{"fragments":["domain.model","checks.model"]}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("domain.model"),
        "model Split\nentity Client \"利用者\"\nscope Client = 1, steps = 2",
    )
    .unwrap();

    for (body, position, message) in [
        (
            "// 型エラー\ninvariant Valid { some Unknown }\n",
            "2:1",
            "Type: unknown name Unknown",
        ),
        (
            "// 構文エラー\n  @\n",
            "2:3",
            "Syntax: unexpected character",
        ),
        (
            "// 構文エラー\ninvariant Valid ]\n",
            "2:17",
            "Syntax: expected {",
        ),
    ] {
        std::fs::write(&checks, body).unwrap();
        for generate in [false, true] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_rcheck"));
            cmd.current_dir(&root);
            if generate {
                cmd.arg("generate");
            }
            cmd.arg("project.rmodel.json");
            if generate {
                cmd.arg("ir");
            }
            let output = cmd.output().unwrap();
            let diagnostic = String::from_utf8(output.stderr).unwrap();
            assert_eq!(output.status.code(), Some(2), "{diagnostic}");
            assert!(
                diagnostic.starts_with(&format!("checks.model:{position}: ")),
                "{diagnostic}"
            );
            assert!(diagnostic.contains(message), "{diagnostic}");
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn one_component_source_can_be_instantiated_twice_with_distinct_state() {
    let project = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/model-patterns/offline-merge/two-stores/project.rmodel.json"
    );
    let result = invoke(&[
        "witness",
        project,
        "IndependentReceipts",
        "--max-states",
        "20000",
    ]);
    assert_eq!(result["verdict"], "reachable");
    assert_eq!(result["complete"], true);
    let state = &result["trace"]["finalState"];
    assert_eq!(state["east_head"], 2);
    assert_eq!(state["west_head"], 1);
    assert_eq!(state["eastDelivery_received"]["First"], 2);
    assert_eq!(state["westDelivery_received"]["First"], 1);
    let ir = invoke(&["generate", project, "ir"]);
    assert!(
        ir["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["name"] == "east_commit")
    );
    assert!(
        ir["actions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["name"] == "west_commit")
    );
}

#[test]
fn project_rejects_unowned_writes_and_keeps_component_source_positions() {
    use serde_json::json;
    let root = std::env::temp_dir().join(format!("rcheck-write-owner-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let manifest = root.join("project.rmodel.json");
    let run = |body: &str, other: serde_json::Value, contract_extra: &str, expected: &str| {
        std::fs::write(
            root.join("contract.model"),
            format!("model Ownership\nvar shared: one Bool\ninit {{ not shared }}\nscope steps = 2\n{contract_extra}"),
        ).unwrap();
        std::fs::write(root.join("part.model"), body).unwrap();
        std::fs::write(
            root.join("writer.model"),
            "behavior @update() { update { @output = true } }",
        )
        .unwrap();
        let project = json!({
            "fragments": [
                "contract.model",
                {"path":"part.model", "namespace":"observer", "ports": {
                    "input":{"state":"shared","access":"read"}
                }},
                other
            ]
        });
        std::fs::write(&manifest, project.to_string()).unwrap();
        let error = rcheck::project::load_source(manifest.to_str().unwrap()).unwrap_err();
        assert!(error.contains(expected), "{error}");
    };
    let writer = json!({
        "path":"writer.model","namespace":"writer","ports":{
            "output":{"state":"shared","access":"write"}
        }
    });
    for body in [
        "// 日本語 @ignored\nbehavior @clear() { update { @input = false } }",
        "// 日本語 @ignored\nbehavior @clear() { update { shared = false } }",
    ] {
        run(
            body,
            writer.clone(),
            "",
            "part.model:2:1: observer_clear cannot write shared",
        );
    }
    run(
        "var @local: one Bool\ninit { not @local }",
        writer.clone(),
        "behavior spoof() { update { observer_local = true } }",
        "contract.model:5:1: spoof cannot write observer_local",
    );
    run(
        "behavior @watch() { guard { @input } }",
        writer.clone(),
        "behavior spoof() { update { shared = true } }",
        "contract.model:5:1: spoof cannot write shared",
    );
    run(
        "// 日本語\nbehavior @watch() ]",
        writer.clone(),
        "",
        "part.model:2:19: Syntax: expected {",
    );
    let mut second_writer = writer.clone();
    second_writer["namespace"] = json!("other");
    std::fs::write(
        &manifest,
        json!({"fragments": ["contract.model", writer, second_writer]}).to_string(),
    )
    .unwrap();
    let error = rcheck::project::load_model_source(manifest.to_str().unwrap()).unwrap_err();
    assert!(
        error.contains("shared has two write ports, other and writer"),
        "{error}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn component_reads_only_local_state_or_declared_ports() {
    use serde_json::json;

    let root = std::env::temp_dir().join(format!("rcheck-read-owner-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let manifest = root.join("project.rmodel.json");
    std::fs::write(
        root.join("contract.model"),
        "model Reads\nrecord Event { left: Bool }\nvar left: one Bool\nvar right: one Bool\n\
         rel ready: one Bool\nderive leak = left\ninit { left right }\nscope steps = 3\n\
         invariant ProjectCanReadPrivateState { observer_local }",
    )
    .unwrap();
    std::fs::write(
        root.join("other.model"),
        "var @private: one Bool\ninit { @private }",
    )
    .unwrap();
    std::fs::write(
        &manifest,
        json!({"fragments":[
            "contract.model",
            {"path":"other.model", "namespace":"other"},
            {"path":"part.model", "namespace":"observer", "ports":{
                "input":{"state":"right","access":"read"}
            }}
        ]})
        .to_string(),
    )
    .unwrap();
    let part = root.join("part.model");
    let local = "var @local: one Bool\ninit { @local }\n";
    std::fs::write(
        &part,
        format!("{local}behavior @accept(event: Event) {{ guard {{ @input and @local and ready and event.left }} }}"),
    )
    .unwrap();
    let path = manifest.to_str().unwrap();
    let result = invoke(&[path, "--question", "ProjectCanReadPrivateState"]);
    assert_eq!(result[0]["answers"][0]["verdict"], "holds");
    for (body, state) in [
        ("behavior @accept() { guard { left } }", "left"),
        (
            "behavior @accept() { guard { other_private } }",
            "other_private",
        ),
        ("behavior @accept() { guard { leak } }", "left"),
        (
            "behavior @accept() { update { @local = if { left } then true else false } }",
            "left",
        ),
        (
            "behavior @accept() { update { let hidden = left @local = true } }",
            "left",
        ),
        ("derive @hidden = left", "left"),
        ("invariant @Safe { left }", "left"),
        ("eventually @Progress { left }", "left"),
        ("transition @Safe { after(left) }", "left"),
        ("init { left }", "left"),
    ] {
        std::fs::write(&part, format!("{local}{body}")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .args(["generate", path, "ir"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{body} read {state}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains(&format!("part.model:3:1: observer cannot read {state}")),
            "{body}: {error}"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
