use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/model-patterns/offline-merge/two-stores")
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .args(args)
        .output()
        .unwrap()
}

fn report(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn rejects(args: &[&str], message: &str) {
    let output = run(args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(stderr.contains(message), "{stderr}");
}

#[test]
fn component_types_can_be_checked_without_a_host() {
    for (name, access, action, guarantee) in [
        ("storage", "write", "commit", "ReceiptStable"),
        ("delivery", "read", "send", "DeliveredFromReceipt"),
    ] {
        let component = example().join(format!("{name}.component.json"));
        let result = report(&["component", component.to_str().unwrap()]);
        assert_eq!(
            json!({"status":result["status"], "ports":result["ports"], "operations":result["operations"]}),
            json!({
                "status":"typed",
                "ports":{"receipt":{"shape":"Request -> lone Nat","access":access}},
                "operations":[action],
            })
        );
        assert!(
            result["contract"]["guarantees"]
                .as_array()
                .unwrap()
                .contains(&json!(guarantee))
        );
    }
    let manifest = example().join("model.project.json");
    let result = report(&["witness", manifest.to_str().unwrap(), "IndependentReceipts"]);
    assert_eq!(result["complete"], true);
    assert_eq!(result["trace"]["commands"].as_array().unwrap().len(), 5);
}

#[test]
fn ideal_sources_work_as_component_and_project_fragments() {
    let directory = std::env::temp_dir().join(format!("rcheck-ideal-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "request-types.ideal",
        "receipt-interface.ideal",
        "delivery.ideal",
        "delivery.component.json",
    ] {
        std::fs::copy(example().join(name), directory.join(name)).unwrap();
    }
    let component = directory.join("delivery.component.json");
    let typed = report(&["component", component.to_str().unwrap()]);
    assert_eq!(typed["status"], "typed");

    std::fs::write(
        directory.join("host.ideal"),
        "model Host\nvar inbox: Request -> lone Nat\nscope Int = 0..2, steps = 4",
    )
    .unwrap();
    let project = directory.join("host.project.json");
    std::fs::write(
        &project,
        json!({"fragments": [
            "host.ideal", "request-types.ideal",
            {"component":"delivery.component.json", "namespace":"subscriber", "ports":{"receipt":"inbox"}}
        ]})
        .to_string(),
    )
    .unwrap();
    let result = report(&[project.to_str().unwrap()]);
    assert_eq!(result[0]["answers"][0]["verdict"], "holds");
    let legacy = directory.join("host.rmodel.json");
    std::fs::copy(project, &legacy).unwrap();
    assert_eq!(report(&[legacy.to_str().unwrap()]), result);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn project_link_checks_the_declared_port_shape_and_shared_types() {
    let directory = std::env::temp_dir().join(format!("rcheck-component-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "request-types.ideal",
        "receipt-interface.ideal",
        "delivery.component.json",
        "delivery.ideal",
    ] {
        std::fs::copy(example().join(name), directory.join(name)).unwrap();
    }
    let root = directory.join("host.model");
    let project = directory.join("host.rmodel.json");
    let try_project = [project.to_str().unwrap()];
    let instance = json!({"component":"delivery.component.json","namespace":"subscriber","ports":{"receipt":"inbox"}});
    let manifest =
        |types: &str, instance: &Value| json!({"fragments":["host.model",types,instance]});
    let write_manifest = |types: &str, instance: &Value| {
        std::fs::write(&project, manifest(types, instance).to_string()).unwrap()
    };
    write_manifest("request-types.ideal", &instance);
    std::fs::write(
        &root,
        "model Host\nvar inbox: Request -> lone Nat\nscope Int = 0..2, steps = 4",
    )
    .unwrap();
    assert_eq!(report(&try_project)[0]["answers"][0]["verdict"], "holds");

    std::fs::write(
        &root,
        "model Host\nvar inbox: Request -> one Nat\nscope Int = 0..2, steps = 4",
    )
    .unwrap();
    rejects(
        &try_project,
        "subscriber.receipt requires Request -> lone Nat, but inbox has Request -> one Nat",
    );
    std::fs::write(
        &root,
        "model Host\nvar inbox: Request -> lone Nat\nscope Int = 0..2, steps = 4",
    )
    .unwrap();
    std::fs::write(
        directory.join("lookalike.model"),
        "enum Request = First | Third",
    )
    .unwrap();
    write_manifest("lookalike.model", &instance);
    rejects(&try_project, "subscriber requires a shared type fragment");
    write_manifest(
        "request-types.ideal",
        &json!({"component":"delivery.component.json","namespace":"subscriber","ports":{}}),
    );
    rejects(&try_project, "subscriber is missing port receipt");
    write_manifest("request-types.ideal", &instance);
    let body = directory.join("delivery.ideal");
    std::fs::write(
        &body,
        "behavior @send(request: Request) { update { @receipt = none } }",
    )
    .unwrap();
    let component = directory.join("delivery.component.json");
    rejects(
        &["component", component.to_str().unwrap()],
        "delivery.ideal:1:1: part_send cannot write receipt",
    );
    rejects(
        &try_project,
        "delivery.ideal:1:1: part_send cannot write receipt",
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_type_correct_receipt_writer_can_still_break_a_linked_contract() {
    let manifest = example().join("model.project.json");
    let good = report(&["contracts", manifest.to_str().unwrap()]);
    assert_eq!(good["status"], "matched");
    let subscriber = &good["components"]["westDelivery"];
    assert_eq!(
        subscriber["assumptions"][0]["check"],
        "westDelivery_StableInput"
    );
    assert_eq!(subscriber["assumptions"][0]["verdict"], "holds");
    assert_eq!(subscriber["guarantees"][0]["verdict"], "holds");
    assert_eq!(subscriber["exercises"][0]["verdict"], "reachable");

    let directory =
        std::env::temp_dir().join(format!("rcheck-receipt-contract-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for file in std::fs::read_dir(example()).unwrap() {
        let file = file.unwrap();
        std::fs::copy(file.path(), directory.join(file.file_name())).unwrap();
    }
    let storage = directory.join("storage.ideal");
    let mut changed = std::fs::read_to_string(&storage).unwrap();
    changed.push_str("\nbehavior @forget(request: Request) { guard { some request.@receipt } update { @receipt = @receipt - request -> request.@receipt } }\n");
    std::fs::write(storage, changed).unwrap();
    let typed = report(&[
        "component",
        directory.join("storage.component.json").to_str().unwrap(),
    ]);
    assert_eq!(typed["status"], "typed");
    let output = run(&[
        "contracts",
        directory.join("model.project.json").to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let broken: Value = serde_json::from_slice(&output.stdout).unwrap();
    let subscriber = &broken["components"]["westDelivery"];
    let assumption = &subscriber["assumptions"][0];
    assert_eq!(assumption["verdict"], "violated");
    assert_eq!(
        assumption["evidence"]["trace"]["commands"][1]["kind"],
        "west_forget"
    );
    assert_eq!(subscriber["guarantees"][0]["verdict"], "violated");
    assert_eq!(subscriber["status"], "assumption_failed");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_finite_harness_supplies_input_and_exposes_vacuous_delivery() {
    let harness = example().join("delivery-harness.project.json");
    let good = report(&["contracts", harness.to_str().unwrap()]);
    let exercise = &good["components"]["subscriber"]["exercises"][0];
    let commands = exercise["evidence"]["trace"]["commands"]
        .as_array()
        .unwrap();
    assert_eq!(commands[0]["kind"], "publish");
    assert_eq!(commands[1]["kind"], "subscriber_send");
    assert_eq!(commands[0]["request"], commands[1]["request"]);

    let limit = run(&["contracts", harness.to_str().unwrap(), "--max-states", "1"]);
    assert_eq!(limit.status.code(), Some(3), "{limit:?}");
    let limited: Value = serde_json::from_slice(&limit.stdout).unwrap();
    assert_eq!(
        limited["components"]["subscriber"]["status"],
        "inconclusive"
    );
    assert_eq!(limited["diagnostics"][0]["resource"], "states");

    let directory =
        std::env::temp_dir().join(format!("rcheck-delivery-harness-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for name in [
        "delivery-harness.project.json",
        "delivery.ideal",
        "delivery.component.json",
        "request-types.ideal",
        "receipt-interface.ideal",
    ] {
        std::fs::copy(example().join(name), directory.join(name)).unwrap();
    }
    std::fs::write(directory.join("delivery-harness.ideal"), "model NoDelivery\nvar input: Request -> lone Nat\ninit { no input }\nscope Int = 0..2, steps = 6\n").unwrap();
    let manifest = directory.join("delivery-harness.project.json");
    let no_input = run(&["contracts", manifest.to_str().unwrap()]);
    assert_eq!(no_input.status.code(), Some(1), "{no_input:?}");
    let no_input: Value = serde_json::from_slice(&no_input.stdout).unwrap();
    assert_eq!(
        no_input["components"]["subscriber"]["assumptions"][0]["verdict"],
        "holds"
    );
    assert_eq!(
        no_input["components"]["subscriber"]["guarantees"][0]["verdict"],
        "holds"
    );
    assert_eq!(
        no_input["components"]["subscriber"]["status"],
        "exercise_unreachable"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
