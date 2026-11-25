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
