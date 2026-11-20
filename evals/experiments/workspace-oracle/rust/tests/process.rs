use serde_json::json;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(kind: &str, mode: &str) -> (bool, serde_json::Value) {
    let root = std::env::temp_dir().join(format!(
        "roracle-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_workspace-oracle");
    let candidate = if kind == "missing" {
        "/nonexistent/roracle"
    } else {
        binary
    };
    let request = if kind == "echo" {
        json!({"kind":"fixture","status":"complete","data":(0..4096).map(|index| json!({"index":index,"value":"large payload across buffer boundaries"})).collect::<Vec<_>>()})
    } else {
        json!({})
    };
    let reference = if kind == "echo" { "echo" } else { "stable" };
    let manifest = json!({"version":1,"name":"process-control","mode":mode,
        "baseline":{"path":binary,"args":["--fixture",reference]},"candidate":{"path":candidate,"args":["--fixture",kind]},
        "cases":[{"id":"input","request":request,"requirements":[{"pointer":"/kind","value":"fixture"},{"pointer":"/status","value":"complete"}]}],
        "config":{"baseline":"baseline","scope":"process test","samples":2,"warmup":1,"iterations":1,"max_evaluations":20}});
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let result = Command::new(binary)
        .arg(root.join("manifest.json"))
        .arg(root.join("run"))
        .output()
        .unwrap();
    let report =
        serde_json::from_slice(&std::fs::read(root.join("run/bench.json")).unwrap()).unwrap();
    std::fs::remove_dir_all(root).unwrap();
    (result.status.success(), report)
}
#[test]
fn real_process_failures_malformed_json_and_drift_never_produce_speedups() {
    for (kind, status) in [
        ("corrupt", "mismatch"),
        ("exit", "error"),
        ("bad-json", "error"),
        ("drift", "mismatch"),
        ("exit-after", "error"),
        ("unknown", "unknown"),
    ] {
        let (success, report) = run(kind, "reused");
        assert!(!success, "{kind}");
        assert_eq!(report["status"], status, "{kind}");
        for measurement in report["cases"][0]["measurements"].as_array().unwrap() {
            assert!(measurement["speedup"].is_null());
        }
    }
}
#[test]
fn independent_cold_and_reused_processes_have_matching_complete_outputs() {
    for mode in ["cold", "reused"] {
        let (success, report) = run("stable", mode);
        assert!(success, "{report}");
        assert_eq!(report["evaluations"], report["planned_evaluations"]);
        assert_eq!(report["status"], "matched");
    }
}
#[test]
fn missing_program_is_an_execution_error() {
    let (success, report) = run("missing", "cold");
    assert!(!success);
    assert_eq!(report["status"], "error");
}

#[test]
fn invalid_manifest_is_rejected_before_spawning_an_implementation() {
    let root = std::env::temp_dir().join(format!("roracle-invalid-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let input = json!({"version":2,"name":"invalid","mode":"reused","baseline":{"path":"/missing","args":[]},"candidate":{"path":"/missing","args":[]},"cases":[],"config":{"baseline":"baseline","scope":"test","samples":1,"warmup":0,"iterations":1,"max_evaluations":10}});
    std::fs::write(root.join("input.json"), serde_json::to_vec(&input).unwrap()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_workspace-oracle"))
        .arg(root.join("input.json"))
        .arg(root.join("output"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("invalid runner version")
    );
    assert!(!root.join("output").exists());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn large_payloads_are_flushed_and_preserved_before_reading_a_reply() {
    for mode in ["cold", "reused"] {
        let (success, report) = run("echo", mode);
        assert!(success, "{report}");
        assert_eq!(
            report["preflight"]["cases"][0]["reference"]["value"]["data"]
                .as_array()
                .unwrap()
                .len(),
            4096
        );
        assert_eq!(report["status"], "matched");
    }
}
