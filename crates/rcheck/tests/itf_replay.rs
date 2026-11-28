use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bank() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rcheck/bank.model")
        .canonicalize()
        .unwrap()
}

fn run(directory: &Path, args: &[&str], status: i32) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(status),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn replay(directory: &Path, trace: &Value, status: i32) -> Output {
    std::fs::write(directory.join("saved.itf.json"), trace.to_string()).unwrap();
    run(
        directory,
        &["replay-itf", bank().to_str().unwrap(), "saved.itf.json"],
        status,
    )
}

#[test]
fn a_single_observation_replays_the_initial_state_without_commands() {
    let directory = std::env::temp_dir().join(format!("rcheck-itf-init-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let model = bank();
    let initial = json!({"balance": {"Account0": 0, "Account1": 0}});
    let mut recording = json!({
        "config":{}, "initial":initial, "commands":[], "observations":[initial]
    });
    let path = directory.join("recording.json");
    std::fs::write(&path, recording.to_string()).unwrap();
    let recorded = run(
        &directory,
        &["record-itf", model.to_str().unwrap(), "recording.json"],
        0,
    );
    let mut trace: Value = serde_json::from_slice(&recorded.stdout).unwrap();
    let matched: Value = serde_json::from_slice(&replay(&directory, &trace, 0).stdout).unwrap();
    assert_eq!(matched, json!({"verdict":"matched","steps":0}));
    trace["states"][0]["observation"]["balance"]["Account0"] = json!({"#bigint":"1"});
    let mismatch: Value = serde_json::from_slice(&replay(&directory, &trace, 1).stdout).unwrap();
    assert_eq!(mismatch["difference"]["step"], 0);
    assert!(mismatch["difference"]["command"].is_null());
    trace["states"] = json!([]);
    replay(&directory, &trace, 2);
    recording["observations"] = json!([]);
    std::fs::write(&path, recording.to_string()).unwrap();
    run(
        &directory,
        &["record-itf", model.to_str().unwrap(), "recording.json"],
        2,
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn portable_replay_distinguishes_a_difference_missing_evidence_and_invalid_commands() {
    let directory = std::env::temp_dir().join(format!("rcheck-itf-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let initial = json!({"balance": {"Account0": 0, "Account1": 0}});
    let recorded = json!({
        "config":{}, "initial":initial,
        "commands":[{"kind":"deposit","a":"Account0","n":2},{"kind":"withdraw","a":"Account0","n":1}],
        "observations":[initial,{"balance":{"Account0":2,"Account1":0}},{"balance":{"Account0":1,"Account1":0}}]
    });
    std::fs::write(directory.join("recording.json"), recorded.to_string()).unwrap();
    let encoded = run(
        &directory,
        &["record-itf", "recording-host/bank.model", "recording.json"],
        0,
    );
    let mut trace: Value = serde_json::from_slice(&encoded.stdout).unwrap();
    assert_eq!(
        trace["states"][1]["observation"]["balance"]["Account0"],
        json!({"#bigint":"2"})
    );
    let ordinary: Value = serde_json::from_slice(&replay(&directory, &trace, 0).stdout).unwrap();
    assert_eq!(ordinary, json!({"verdict":"matched","steps":2}));

    trace["states"].as_array_mut().unwrap().pop();
    let missing = replay(&directory, &trace, 2);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing observation at step 2"));

    trace["states"][1]["observation"]["balance"]["Account0"]["#bigint"] = json!("3");
    let different: Value = serde_json::from_slice(&replay(&directory, &trace, 1).stdout).unwrap();
    assert_eq!(different["verdict"], "mismatch");
    assert_eq!(different["difference"]["step"], 1);
    assert_eq!(
        different["difference"]["expected"]["balance"]["Account0"],
        2
    );
    assert_eq!(
        different["difference"]["observed"]["balance"]["Account0"],
        3
    );

    trace["states"][1]["command"]["n"]["#bigint"] = json!("0");
    let inconsistent = replay(&directory, &trace, 2);
    assert!(
        String::from_utf8_lossy(&inconsistent.stderr).contains("inconsistent ITF replay frame 1")
    );
    trace["#meta"]["rmodelReplay"]["commands"][0]["n"]["#bigint"] = json!("0");
    let disabled = replay(&directory, &trace, 2);
    assert!(String::from_utf8_lossy(&disabled.stderr).contains("step 1: guard is false"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn itf_preserves_integer_precision_and_sequence_order() {
    let directory = std::env::temp_dir().join(format!("rcheck-itf-values-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("model.model"),
        r#"model Packet
enum Byte = A | B
sequence PacketBytes = Byte, max = 2
var offset: one Nat
var packet: one PacketBytes
init { offset = 9007199254740993 packet = PacketBytes[A, B] }
behavior receive() {
  guard { offset = 9007199254740993 }
  update { offset = offset + 1 }
}
scope Int = 9007199254740993..9007199254740994, steps = 2
"#,
    )
    .unwrap();
    let recording = json!({
        "config":{}, "initial":{"offset":9007199254740993_i64,"packet":{"$sequence":["A","B"]}},
        "commands":[{"kind":"receive"}],
        "observations":[
            {"offset":9007199254740993_i64,"packet":{"$sequence":["A","B"]}},
            {"offset":9007199254740994_i64,"packet":{"$sequence":["A","B"]}}
        ]
    });
    std::fs::write(directory.join("recording.json"), recording.to_string()).unwrap();
    let encoded = run(&directory, &["record-itf", "Packet", "recording.json"], 0);
    let mut trace: Value = serde_json::from_slice(&encoded.stdout).unwrap();
    assert_eq!(
        trace["states"][1]["observation"]["offset"],
        json!({"#bigint":"9007199254740994"})
    );
    let run_trace = |trace: &Value, status| {
        std::fs::write(directory.join("saved.itf.json"), trace.to_string()).unwrap();
        run(
            &directory,
            &["replay-itf", "model.model", "saved.itf.json"],
            status,
        )
    };
    run_trace(&trace, 0);
    trace["states"][1]["observation"]["packet"]["$sequence"] = json!(["B", "A"]);
    let result: Value = serde_json::from_slice(&run_trace(&trace, 1).stdout).unwrap();
    assert_eq!(
        result["difference"]["expected"]["packet"]["$sequence"],
        json!(["A", "B"])
    );
    trace["states"][1]["observation"]["offset"]["#bigint"] = json!("000");
    assert!(
        String::from_utf8_lossy(&run_trace(&trace, 2).stderr)
            .contains("invalid ITF replay integer")
    );
    std::fs::remove_dir_all(directory).unwrap();
}
