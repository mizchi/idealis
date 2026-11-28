use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const EXAMPLE: &str = "examples/model-patterns/library-membership";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn impact(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .current_dir(root())
        .arg("impact")
        .args(args)
        .output()
        .unwrap()
}

fn names(report: &Value, field: &str) -> Vec<String> {
    report[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap().into())
        .collect()
}

#[test]
fn reverse_lookup_uses_resolved_component_ports_and_links_rules() {
    let directory = std::env::temp_dir().join(format!("rcheck-impact-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let spec = directory.join("spec.json");
    std::fs::write(
        &spec,
        json!({"spec": {
            "id": "library",
            "name": {"ja": "貸出", "en": "Lending"},
            "groups": [],
            "terms": [],
            "rules": [
                {
                    "id": "LIBRARY-ACTIVE-AT-BORROW",
                    "text": {"ja": "貸出には資格が必要。", "en": "Borrowing requires eligibility."},
                    "terms": [],
                    "claims": [
                        {"model": "notification.project.json", "check": "BorrowRequiresActive", "expect": "holds"},
                        {"model": "direct.project.json", "check": "BorrowRequiresActive", "expect": "holds"}
                    ]
                },
                {
                    "id": "OTHER-MODEL",
                    "text": {"ja": "直接参照だけの規則。", "en": "A direct-only rule."},
                    "terms": [],
                    "claims": [{"model": "direct.project.json", "check": "BorrowRequiresActive", "expect": "holds"}]
                }
            ],
            "groundings": [{
                "model": "notification.project.json",
                "implementation": {"path": "library.go", "symbol": "Library"},
                "test": {"path": "library_test.go", "symbol": "TestNotification"}
            }]
        }})
        .to_string(),
    )
    .unwrap();
    let direct = format!("{EXAMPLE}/direct.project.json");
    let notify = format!("{EXAMPLE}/notification.project.json");
    let options = ["--spec", spec.to_str().unwrap(), "--root", EXAMPLE];
    let query = |model, state| {
        let args = [&[model, state], options.as_slice()].concat();
        let output = impact(&args);
        assert!(output.status.success(), "{output:?}");
        serde_json::from_slice::<Value>(&output.stdout).unwrap()
    };

    let report = query(&direct, "activeMembers");
    assert_eq!(
        names(&report, "actions"),
        ["membership_suspend", "library_borrow"]
    );
    let suspended = &report["actions"][0];
    assert_eq!(suspended["reads"], true);
    assert_eq!(suspended["writes"], true);
    assert!(
        suspended["location"]
            .as_str()
            .unwrap()
            .contains("membership.ideal:")
    );
    assert_eq!(report["actions"][1]["writes"], false);
    assert_eq!(report["checks"][0]["reads"], true);
    assert_eq!(report["groundings"], json!([]));

    let authority = query(&notify, "activeMembers");
    assert_eq!(
        names(&authority, "actions"),
        ["deliverSuspension", "membership_suspend"]
    );
    assert_eq!(names(&authority, "checks"), ["BorrowRequiresActive"]);

    let cache = query(&notify, "knownActiveMembers");
    assert_eq!(
        names(&cache, "actions"),
        ["deliverSuspension", "library_borrow"]
    );
    assert_eq!(
        names(&cache, "checks"),
        ["BorrowRequiresActive", "BorrowRequiresKnownEligibility"]
    );
    assert_eq!(cache["checks"][0]["reads"], false);
    assert_eq!(cache["checks"][0]["viaActions"], json!(["library_borrow"]));
    assert_eq!(
        cache["rules"],
        json!([{"id": "LIBRARY-ACTIVE-AT-BORROW", "checks": ["BorrowRequiresActive"]}])
    );
    assert_eq!(cache["groundings"][0]["test"]["symbol"], "TestNotification");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unknown_variables_report_a_model_error() {
    let model = format!("{EXAMPLE}/direct.project.json");
    let output = impact(&[&model, "eligible"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown state variable eligible"));
}

#[test]
fn derived_reads_and_fairness_links_point_to_their_source() {
    let model = std::env::temp_dir().join(format!(
        "rcheck-impact-derived-{}.ideal",
        std::process::id()
    ));
    std::fs::write(
        &model,
        r#"
model Relay
enum Bit = Off | On
var source: one Bit
var output: one Bit
init { source = Off, output = Off }
derive signal = source
behavior emit() { update { output = signal } }
behavior clear() { update { source = Off } }
eventually Delivered with { weak emit } { output = On }
scope steps = 4
"#,
    )
    .unwrap();
    let queried = impact(&[model.to_str().unwrap(), "source"]);
    std::fs::remove_file(model).unwrap();
    assert!(queried.status.success(), "{queried:?}");
    let report: Value = serde_json::from_slice(&queried.stdout).unwrap();
    assert_eq!(names(&report, "actions"), ["emit", "clear"]);
    assert_eq!(report["actions"][0]["reads"], true);
    assert_eq!(report["actions"][1]["reads"], false);
    assert_eq!(report["actions"][1]["writes"], true);
    assert_eq!(names(&report, "derivations"), ["signal"]);
    assert_eq!(names(&report, "checks"), ["Delivered"]);
    assert_eq!(report["checks"][0]["viaActions"], json!(["emit"]));
}
