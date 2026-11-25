use serde_json::{Value, json};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const MODEL: &str = "
model Gate
enum Position = Open | Closed
var position: one Position
init { position = Closed }

behavior unlock() {
  guard { position = Closed }
  update { position = Open }
}
behavior enter() { guard { position = Open } }
behavior jam() { guard { position = Open position = Closed } }
scope steps = 3
";

fn invoke(source: &str, extension: &str, args: &[&str]) -> Output {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "rcheck-dead-code-{}-{}.{extension}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .arg("dead-code")
        .arg(&path)
        .args(args)
        .output();
    std::fs::remove_file(path).unwrap();
    output.unwrap()
}

fn decode(output: Output) -> (Option<i32>, Value) {
    let report = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code(), report)
}

fn run(model: &str, args: &[&str]) -> (Option<i32>, Value) {
    decode(invoke(model, "model", args))
}

fn report(coverage: &str, statuses: [&str; 3]) -> Value {
    json!({
        "coverage": coverage,
        "actions": [
            { "name": "unlock", "status": statuses[0] },
            { "name": "enter", "status": statuses[1] },
            { "name": "jam", "status": statuses[2] }
        ]
    })
}

#[test]
fn actions_are_checked_in_reachable_states() {
    let isolated_guard = MODEL
        .replace(
            "enum Position = Open | Closed",
            "enum Position = Open | Closed | Jammed",
        )
        .replace("position = Open position = Closed", "position = Jammed");
    for source in [MODEL, isolated_guard.as_str()] {
        assert_eq!(
            run(source, &[]),
            (
                Some(1),
                report("complete", ["reachable", "reachable", "unreachable"])
            )
        );
    }
    let live = MODEL.replace("position = Open position = Closed", "position = Closed");
    assert_eq!(
        run(&live, &[]),
        (
            Some(0),
            report("complete", ["reachable", "reachable", "reachable"])
        )
    );
}

#[test]
fn empty_models_and_partial_search_do_not_claim_dead_code() {
    assert_eq!(
        run(MODEL, &["--max-states", "4"]),
        (
            Some(3),
            report("unknown", ["reachable", "unknown", "unknown"])
        )
    );
    let empty = MODEL.replace(
        "init { position = Closed }",
        "init { position = Closed position = Open }",
    );
    assert_eq!(
        run(&empty, &[]),
        (Some(3), report("empty", ["empty", "empty", "empty"]))
    );
}

#[test]
fn scope_exits_leave_unseen_actions_unknown() {
    let source = "
model Counter
var count: one Int
init { count = 0 }
behavior advance() { guard { count = 0 } update { count = 1 } }
behavior later() { guard { count = 1 } }
scope Int = 0..0, steps = 3
";
    assert_eq!(
        run(source, &[]),
        (
            Some(3),
            json!({
                "coverage": "unknown",
                "actions": [
                    { "name": "advance", "status": "reachable" },
                    { "name": "later", "status": "unknown" }
                ]
            })
        )
    );
}

#[test]
fn complete_ir_agrees_and_omitted_actions_are_rejected() {
    let model = rmodel::parse(MODEL).unwrap();
    let mut ir = rcheck::ir::to_ir(&model).unwrap();
    let source = serde_json::to_string(&ir).unwrap();
    assert_eq!(decode(invoke(&source, "json", &[])), run(MODEL, &[]));
    ir.omitted.push(rcheck::ir::Omitted {
        name: "prepare".into(),
        reason: "unsupported transition".into(),
    });
    let output = invoke(&serde_json::to_string(&ir).unwrap(), "json", &[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("adapter omissions"));
}
