use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn deadlock(model: &Path, args: &[&str], code: i32) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .arg("deadlock")
        .arg(model)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

fn locks(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/worker-locks")
        .join(format!("{name}.rmodel.json"))
}

#[test]
fn lock_contention_is_distinct_from_orderly_completion() {
    let ordered = locks("ordered");
    let result = deadlock(&ordered, &["--unless", "BothFinished"], 0);
    assert_eq!(result["verdict"], "unreachable");
    assert_eq!(result["complete"], true);
    assert!(result["trace"].is_null());

    let terminal = deadlock(&ordered, &[], 1);
    assert_eq!(terminal["verdict"], "reachable");
    assert_eq!(terminal["trace"]["commands"].as_array().unwrap().len(), 6);
    assert_eq!(
        terminal["trace"]["finalState"]["finished"],
        serde_json::json!(["Left", "Right"])
    );

    let reversed = locks("reversed");
    let result = deadlock(&reversed, &["--unless", "BothFinished"], 1);
    assert_eq!(result["verdict"], "reachable");
    assert_eq!(result["complete"], true);
    let trace = &result["trace"];
    let acquisition = trace["commands"].as_array().unwrap();
    assert_eq!(acquisition.len(), 2);
    assert_eq!(acquisition[0]["kind"], "acquire");
    assert_eq!(acquisition[1]["kind"], "acquire");
    assert_ne!(acquisition[0]["worker"], acquisition[1]["worker"]);
    assert_eq!(trace["finalState"]["finished"], serde_json::json!([]));
    let result = deadlock(&reversed, &["--max-arguments", "1"], 3);
    assert_eq!(result["verdict"], "unknown");
    assert_eq!(result["diagnostics"][0]["resource"], "arguments");
    let result = deadlock(&reversed, &["--max-states", "1"], 3);
    assert_eq!(result["verdict"], "unknown");
    assert_eq!(result["diagnostics"][0]["resource"], "states");
}

#[test]
fn a_barrier_stalls_when_only_wait_checks_remain() {
    let normal = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/worker-barrier/reuse.model");
    let directory =
        std::env::temp_dir().join(format!("rcheck-deadlock-barrier-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let missing_peer = directory.join("missing-peer.model");
    let source = std::fs::read_to_string(&normal).unwrap();
    let fault = source.replace(
        "generation != Done worker.rounds = generation",
        "(generation = R1 or worker = Fast) generation != Done worker.rounds = generation",
    );
    assert_ne!(fault, source);
    std::fs::write(&missing_peer, fault).unwrap();

    let result = deadlock(&missing_peer, &["--unless", "EveryonePassed"], 0);
    assert_eq!(result["verdict"], "unreachable");
    let progress = ["--unless", "EveryonePassed", "--ignore-stutter"];
    let result = deadlock(&normal, &progress, 0);
    assert_eq!(result["verdict"], "unreachable");
    let result = deadlock(&missing_peer, &progress, 1);
    assert_eq!(result["verdict"], "reachable");
    assert_eq!(result["complete"], true);
    assert_eq!(result["trace"]["commands"].as_array().unwrap().len(), 5);
    assert_eq!(
        result["trace"]["finalState"],
        serde_json::json!({
            "generation": "R2", "rounds": {"Fast": "R2", "Slow": "R2"},
            "arrived": ["Fast"], "waiting": ["Fast"]
        })
    );
    let result = deadlock(
        &missing_peer,
        &["--ignore-stutter", "--max-arguments", "1"],
        3,
    );
    assert_eq!(result["verdict"], "unknown");
    assert_eq!(result["diagnostics"][0]["resource"], "arguments");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn deadlock_requires_a_reachable_state_and_a_full_guard_check() {
    let directory = std::env::temp_dir().join(format!("rcheck-deadlock-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = directory.join("test.model");
    for (body, status, verdict, diagnostic) in [
        ("behavior tick() {}", 0, "unreachable", None),
        ("behavior tick() { guard { false } }", 1, "reachable", None),
        (
            "var flag: one Bool init { not flag }\n\
             behavior flip() { update { flag = if { flag } then false else true } }",
            0,
            "unreachable",
            None,
        ),
        ("init { false }", 3, "empty", None),
        (
            "var n: one Nat init { n = 0 }\nbehavior tick() { update { n = n + 1 } }",
            3,
            "unknown",
            Some("scope_exit"),
        ),
        (
            "var flag: one Bool init { flag }\nbehavior tick() { update { flag = none } }",
            3,
            "unknown",
            Some("declaration_violation"),
        ),
        (
            "var flag: one Bool var n: one Nat init { not flag, n = 0 }\n\
             behavior finish() { guard { not flag } update { flag = true } }\n\
             behavior escape() { guard { not flag } update { n = n + 1 } }",
            1,
            "reachable",
            Some("scope_exit"),
        ),
    ] {
        std::fs::write(
            &model,
            format!("model State\n{body}\nscope Int = 0..0, steps = 2"),
        )
        .unwrap();
        let result = deadlock(&model, &[], status);
        assert_eq!(result["verdict"], verdict, "{body}");
        assert_eq!(
            result["diagnostics"][0]["kind"].as_str(),
            diagnostic,
            "{body}"
        );
        if verdict == "reachable" {
            if diagnostic.is_some() {
                assert_eq!(result["complete"], false);
                assert_eq!(
                    result["trace"]["commands"],
                    serde_json::json!([{"kind":"finish"}])
                );
            } else {
                assert_eq!(result["trace"]["commands"], serde_json::json!([]));
            }
        }
        if body == "behavior tick() {}" {
            let progress = deadlock(&model, &["--ignore-stutter"], 1);
            assert_eq!(progress["verdict"], "reachable");
            assert_eq!(progress["trace"]["commands"], serde_json::json!([]));
        } else {
            assert_eq!(deadlock(&model, &["--ignore-stutter"], status), result);
        }
    }

    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .arg("deadlock")
        .arg(&model)
        .args(["--unless", "Missing"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("no reachable completion condition named Missing")
    );
    std::fs::remove_dir_all(directory).unwrap();
}
