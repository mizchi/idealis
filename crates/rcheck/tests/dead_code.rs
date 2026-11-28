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
    let mut report = json!({
        "coverage": coverage,
        "actions": [
            { "name": "unlock", "status": statuses[0] },
            { "name": "enter", "status": statuses[1] },
            { "name": "jam", "status": statuses[2] }
        ]
    });
    if coverage == "unknown" {
        report["diagnostics"] = json!([{ "kind": "budget", "resource": "states" }]);
    }
    report
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
        run(MODEL, &["--max-states", "3"]),
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
                "diagnostics": [{ "kind": "scope_exit", "depth": 1 }],
                "actions": [
                    { "name": "advance", "status": "reachable" },
                    { "name": "later", "status": "unknown" }
                ]
            })
        )
    );

    // The guard was reached even though its resulting state is outside scope.
    let (status, result) = run(source, &["--instances", "advance"]);
    assert_eq!(status, Some(3));
    assert_eq!(
        result["instances"],
        json!([{ "action": "advance", "args": {}, "status": "reachable" }])
    );
    let (status, result) = run(source, &["--instances", "later"]);
    assert_eq!(status, Some(3));
    assert_eq!(
        result["instances"],
        json!([{ "action": "later", "args": {}, "status": "unknown" }])
    );
}

#[test]
fn scope_exit_and_exhaustion_are_both_reported() {
    let source = "
model Counter
var count: one Int
init { count = 0 }
behavior escape() { guard { count = 0 } update { count = 2 } }
behavior advance() { guard { count = 0 } update { count = 1 } }
scope Int = 0..1, steps = 3
";
    let (status, report) = run(source, &["--max-states", "3"]);
    assert_eq!(status, Some(3));
    assert_eq!(
        report["diagnostics"],
        json!([
            { "kind": "scope_exit", "depth": 1 },
            { "kind": "budget", "resource": "states" }
        ])
    );
}

#[test]
fn an_unsupported_candidate_domain_names_the_relation() {
    let (status, report) = run(
        "model Wide sig A var links: A -> set A behavior wait() {} scope A = 5, steps = 2",
        &[],
    );
    assert_eq!(status, Some(3));
    assert_eq!(report["coverage"], "unknown");
    assert_eq!(
        report["diagnostics"],
        json!([{ "kind": "enumeration_limit", "relation": "links" }])
    );
    assert_eq!(report["actions"][0]["status"], "unknown");
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

#[test]
fn selected_action_reports_reachability_of_each_argument_binding() {
    let source = "
model Dispatch
enum Worker = Left | Right
enum Task = Work | Cleanup
var done: set Worker
init { no done }
behavior handle(worker: Worker, task: Task) {
  guard { task = Work or (worker = Left and Right in done) }
  update { done = done + worker }
}
scope steps = 3
";
    let (status, result) = run(source, &["--instances", "handle"]);
    assert_eq!(status, Some(1));
    assert_eq!(result["coverage"], "complete");
    assert_eq!(
        result["actions"],
        json!([{ "name": "handle", "status": "reachable" }])
    );
    assert_eq!(
        result["instances"],
        json!([
            { "action": "handle", "args": { "worker": "Left", "task": "Work" }, "status": "reachable" },
            { "action": "handle", "args": { "worker": "Left", "task": "Cleanup" }, "status": "reachable" },
            { "action": "handle", "args": { "worker": "Right", "task": "Work" }, "status": "reachable" },
            { "action": "handle", "args": { "worker": "Right", "task": "Cleanup" }, "status": "unreachable" }
        ])
    );

    let (status, result) = run(source, &["--max-states", "0", "--instances", "handle"]);
    assert_eq!(status, Some(3));
    assert_eq!(result["coverage"], "unknown");
    assert!(
        result["instances"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["status"] == "unknown")
    );

    let empty = source.replace("init { no done }", "init { no done some done }");
    let (status, result) = run(&empty, &["--instances", "handle"]);
    assert_eq!(status, Some(3));
    assert!(
        result["instances"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["status"] == "empty")
    );

    let invalid = invoke(source, "model", &["--instances", "missing"]);
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("no action missing"));
}

#[test]
fn checkout_preserves_structured_argument_values() {
    let source = include_str!("../../../examples/model-patterns/checkout/checkout.model");
    let (status, report) = run(source, &["--instances", "capture"]);
    assert_eq!(status, Some(1));
    assert_eq!(report["coverage"], "complete");
    assert_eq!(
        report["instances"][0],
        json!({
            "action": "capture",
            "args": { "amount": { "units": 0 }, "order": "Order0" },
            "status": "unreachable"
        })
    );
    assert_eq!(report["instances"][1]["status"], "reachable");
    assert_eq!(report["instances"][2]["status"], "reachable");

    let ir = rcheck::ir::to_ir(&rmodel::parse(source).unwrap()).unwrap();
    assert_eq!(
        decode(invoke(
            &serde_json::to_string(&ir).unwrap(),
            "json",
            &["--instances", "capture"],
        )),
        (status, report)
    );
}

#[test]
fn setup_failures_do_not_claim_that_instances_are_dead() {
    let unsupported =
        "model Wide sig A var links: A -> set A behavior visit(item: A) {} scope A = 5, steps = 2";
    let (status, result) = run(unsupported, &["--instances", "visit"]);
    assert_eq!(status, Some(3));
    assert_eq!(
        result["diagnostics"],
        json!([{ "kind": "enumeration_limit", "relation": "links" }])
    );
    let instances = result["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 5);
    assert!(instances.iter().all(|entry| entry["status"] == "unknown"));

    let wide = "model Wide sig A behavior mix(a: A, b: A, c: A, d: A, e: A, f: A) {} scope A = 5, steps = 2";
    let output = invoke(wide, "model", &["--instances", "mix"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds the 4096 instance limit"));
}

#[test]
fn a_parameter_named_kind_is_distinct_from_the_action_name() {
    let source = "model KindParam enum A = Item behavior choose(kind: A) {} scope steps = 2";
    let (status, report) = run(source, &["--instances", "choose"]);
    assert_eq!(status, Some(0));
    assert_eq!(
        report["instances"],
        json!([
            { "action": "choose", "args": { "kind": "Item" }, "status": "reachable" }
        ])
    );
}

#[test]
fn fixed_initial_values_leave_budget_to_check_the_edit_retry() {
    let source = include_str!("../../../examples/model-patterns/offline-merge/retry.model");
    let budget = ["--max-states", "20000"];
    let (status, actions) = run(source, &budget);
    assert_eq!(status, Some(0));
    assert_eq!(actions["coverage"], "complete");
    assert!(
        actions["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["status"] == "reachable")
    );

    let (status, calls) = run(source, &[budget[0], budget[1], "--instances", "submit"]);
    assert_eq!(status, Some(0));
    assert_eq!(calls["coverage"], "complete");
    assert_eq!(calls["actions"], actions["actions"]);
    let instances = calls["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 18);
    assert!(instances.iter().all(|entry| entry["status"] == "reachable"));
}

#[test]
fn coverage_keeps_all_configurations_allowed_by_a_disjunctive_fact() {
    let source = "
model Configured
enum Choice = First | Second | Third
rel allowed: one Choice
var observed: lone Choice
fact { allowed = First or allowed = Second }
init { no observed }
behavior pick(choice: Choice) {
  guard { choice = allowed }
  update { observed = choice }
}
scope steps = 2
";
    let (status, result) = run(source, &["--instances", "pick"]);
    assert_eq!(status, Some(1));
    assert_eq!(result["coverage"], "complete");
    assert_eq!(
        result["instances"],
        json!([
            { "action": "pick", "args": { "choice": "First" }, "status": "reachable" },
            { "action": "pick", "args": { "choice": "Second" }, "status": "reachable" },
            { "action": "pick", "args": { "choice": "Third" }, "status": "unreachable" }
        ])
    );
}
