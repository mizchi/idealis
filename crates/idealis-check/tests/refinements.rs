use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

const SOURCE: &str =
    include_str!("../../../examples/model-patterns/payment-data/payment-refined.ideal");

fn run(path: &Path, flags: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .arg("refinements")
        .arg(path)
        .args(flags)
        .output()
        .unwrap()
}

#[test]
fn distinguishes_impossible_types_from_a_scope_without_witnesses() {
    let dir = std::env::temp_dir().join(format!("rcheck-refinements-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("payment.ideal");
    std::fs::write(
        &path,
        SOURCE
            .replace("value <= 1000", "value >= 10")
            .replace("data Fee = Nat", "data Fee = Nat where { value < 0 }"),
    )
    .unwrap();

    let output = run(&path, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let answers = report["types"].as_array().unwrap();
    let find = |name| answers.iter().find(|item| item["type"] == name).unwrap();
    assert_eq!(find("Amount")["domain"]["status"], "sat");
    assert!(find("Amount")["domain"]["value"]["value"].as_i64().unwrap() >= 10);
    assert_eq!(find("Amount")["scope"]["status"], "unsat");
    for name in ["Fee", "Request"] {
        assert_eq!(find(name)["domain"]["status"], "unsat");
        assert!(
            find(name)["location"]
                .as_str()
                .unwrap()
                .contains("payment.ideal:")
        );
    }
    let output = run(&path, &["--type", "Amount"]);
    assert!(output.status.success(), "{output:?}");
    let output = run(&path, &["--type", "Amount", "--max-work", "0"]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["types"][0]["domain"]["status"], "unknown");
}

#[test]
fn nested_constraints_and_variant_payloads_round_trip_through_ir() {
    let dir =
        std::env::temp_dir().join(format!("rcheck-refinements-nested-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = dir.join("mixed.ideal");
    std::fs::write(
        &source,
        "\
model Mixed
data Missing = Nat where { value < 0 }
data Cutoff = Int where { value = -4 }
sequence History = Missing, max = 1
variant Mode = Bad(Missing) | Good(Bool)
data Input = { cutoff: Cutoff, value: Int, mode: Mode, history: History }
  where { value <= cutoff.value and value != cutoff.value }
scope Int = -4..0, steps = 1",
    )
    .unwrap();
    let model = idealis_model::parse(&std::fs::read_to_string(&source).unwrap()).unwrap();
    let ir = dir.join("mixed.json");
    let payload = idealis_check::ir::to_ir(&model).unwrap();
    std::fs::write(&ir, serde_json::to_string(&payload).unwrap()).unwrap();

    let report = |path: &Path| {
        let output = run(path, &["--type", "Input"]);
        assert!(output.status.success(), "{output:?}");
        let answer: Value = serde_json::from_slice(&output.stdout).unwrap();
        answer["types"][0].clone()
    };
    let mut from_source = report(&source);
    assert!(
        from_source
            .as_object_mut()
            .unwrap()
            .remove("location")
            .is_some()
    );
    assert_eq!(from_source, report(&ir));
    assert_eq!(from_source["domain"]["status"], "sat");
    assert_eq!(from_source["scope"]["status"], "unsat");
    let value = &from_source["domain"]["value"];
    assert_eq!(value["cutoff"]["value"], -4);
    assert!(value["value"].as_i64().unwrap() < -4);
    assert_eq!(
        value["mode"],
        serde_json::json!({"tag": "Good", "value": false})
    );
    assert_eq!(value["history"], serde_json::json!({"$sequence": []}));
}
