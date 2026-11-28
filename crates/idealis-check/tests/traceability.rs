use serde_json::Value;
use std::path::Path;
use std::process::Command;

const SPEC: &str = "spec/rmodel-design.json";
const MODEL: &str = "spec/models/checker-comparison.model";
const RULE: &str = "RMODEL-RECEIPT-IMMUTABILITY";
const CHECK: &str = "ReceivedResultStays";

fn report(arguments: &[&str], exit: i32) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(["traceability", SPEC])
        .args(arguments)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(exit), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn a_self_model_counterexample_points_back_to_the_original_rule() {
    let forward = report(&["--rule", RULE, "--document", "spec/rmodel-design.md"], 0);
    assert_eq!(forward["rules"][0]["id"], RULE);
    assert!(forward["rules"][0]["document"]["line"].is_u64());
    assert_eq!(forward["rules"][0]["claims"][0]["check"], CHECK);
    assert_eq!(forward["rules"][0]["claims"][0]["status"], "matched");

    let root = std::env::temp_dir().join(format!("rcheck-traceability-{}", std::process::id()));
    let model = root.join(MODEL);
    std::fs::create_dir_all(model.parent().unwrap()).unwrap();
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(MODEL),
    )
    .unwrap();
    let changed = source.replacen(" and c not in r.received", "", 1);
    assert_ne!(changed, source);
    std::fs::write(&model, changed).unwrap();
    let reverse = report(
        &["--check", MODEL, CHECK, "--root", root.to_str().unwrap()],
        1,
    );
    assert_eq!(reverse["rules"][0]["id"], RULE);
    assert_eq!(reverse["rules"][0]["text"], forward["rules"][0]["text"]);
    let claim = &reverse["rules"][0]["claims"][0];
    assert_eq!(claim["status"], "mismatch");
    assert_eq!(claim["expected"], "holds");
    assert_eq!(claim["evidence"]["verdict"], "violated");
    let trace = &claim["evidence"]["trace"];
    let commands = trace["commands"].as_array().unwrap();
    let (last, prefix) = commands.split_last().unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(last["kind"], "receiveA");
    let request = root.join("validate.json");
    std::fs::write(&request, serde_json::json!({
        "config": trace["config"], "initial": trace["initial"], "prefix": prefix, "command": last,
    }).to_string()).unwrap();
    let replay = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .args([
            "validate",
            model.to_str().unwrap(),
            request.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(replay.status.code(), Some(2), "{replay:?}");
    assert!(
        String::from_utf8(replay.stderr)
            .unwrap()
            .contains(&format!("transition {CHECK}"))
    );

    let stale_document = root.join("stale.md");
    std::fs::write(&stale_document, format!("### {RULE}\n\n別の規則。\n\n")).unwrap();
    let stale = Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args([
            "traceability",
            SPEC,
            "--rule",
            RULE,
            "--document",
            stale_document.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(stale.status.code(), Some(2), "{stale:?}");
    assert!(
        String::from_utf8(stale.stderr)
            .unwrap()
            .contains("differs from the specification")
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unlinked_rules_and_truncated_searches_stay_inconclusive() {
    let unlinked = report(&["--rule", "RMODEL-LEAN-ORACLE"], 3);
    assert_eq!(unlinked["rules"][0]["status"], "unlinked");
    let limited = report(&["--check", MODEL, CHECK, "--max-states", "1"], 3);
    assert_eq!(limited["rules"][0]["claims"][0]["status"], "unknown");
}

#[test]
fn a_clause_link_preserves_the_sentence_and_its_parent_rule() {
    let linked = report(
        &[
            "--check",
            "fixtures/rcheck/structured/shipped-transition.model",
            "KeepsTracking",
        ],
        0,
    );
    assert_eq!(linked["rules"][0]["id"], "SHIPMENT-TRACKING");
    let claim = &linked["rules"][0]["claims"][0];
    assert_eq!(claim["clause"]["id"], "tracking");
    assert!(!claim["clause"]["text"]["ja"].as_str().unwrap().is_empty());
    assert_eq!(claim["evidence"]["verdict"], "holds");
}
