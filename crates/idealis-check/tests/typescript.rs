//! Plan 0102: TypeScript implementations, written against the generated
//! types and served as JSON Lines drivers, conform to the same reference
//! semantics as the Rust ones, and the same faults are found. They run
//! from `dist/`: `just idealis-check-typescript` builds them first.
use idealis_check::conform::{Budget, Process, Status, conform};

#[path = "support/bank_trace.rs"]
mod bank_trace;

fn process(driver: &str, fault: &str) -> Process {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let script = root.join(format!("dist/tests/idealis-check/{driver}.driver.js"));
    assert!(
        script.exists(),
        "{} is missing: run just build",
        script.display()
    );
    Process::spawn("node", &[script.to_str().unwrap(), fault]).unwrap()
}

fn run(model: &str, driver: &str, fault: &str) -> idealis_check::conform::Report {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(root.join(model)).unwrap();
    let model = idealis_model::parse(&source).unwrap();
    conform(&model, &mut process(driver, fault), Budget::default())
}

#[test]
#[ignore = "needs the TypeScript build; run by just idealis-check-typescript"]
fn typescript_implementations_conform() {
    for (model, driver) in [
        ("fixtures/rmodel/permissions.model", "permissions"),
        ("fixtures/rmodel/sessions.model", "sessions"),
        ("fixtures/rcheck/bank.model", "bank"),
    ] {
        let report = run(model, driver, "none");
        assert_eq!(report.status, Status::Matched, "{driver}: {report:?}");
        println!(
            "{driver}: {} configurations, {} states, {} transitions, {} calls",
            report.configurations, report.states, report.transitions, report.calls
        );
    }
}

#[test]
#[ignore = "needs the TypeScript build; run by just idealis-check-typescript"]
fn typescript_replays_the_bank_snapshots() {
    bank_trace::assert_bank_trace(&mut process("bank", "none"));
}

#[test]
#[ignore = "needs the TypeScript build; run by just idealis-check-typescript"]
fn typescript_faults_are_found() {
    for (model, driver, fault, length) in [
        (
            "fixtures/rmodel/permissions.model",
            "permissions",
            "read",
            1,
        ),
        (
            "fixtures/rmodel/permissions.model",
            "permissions",
            "close",
            2,
        ),
        ("fixtures/rmodel/sessions.model", "sessions", "login", 3),
        ("fixtures/rcheck/bank.model", "bank", "withdraw", 2),
    ] {
        let report = run(model, driver, fault);
        assert_eq!(
            report.status,
            Status::Mismatch,
            "{driver} {fault}: {report:?}"
        );
        let failure = report.failure.unwrap();
        assert_eq!(failure.path.len(), length, "{driver} {fault}: {failure:?}");
    }
}
