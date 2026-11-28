//! Plan 0104: Go implementations, written against the generated
//! interfaces and served as JSON Lines drivers, conform to the reference
//! semantics, and the 0102 faults are found. `just idealis-check-go` builds the
//! drivers into .artifacts/idealis-check-go first.
use idealis_check::conform::{Budget, Process, Status, conform};

fn run(model: &str, driver: &str, fault: &str) -> idealis_check::conform::Report {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(root.join(model)).unwrap();
    let model = idealis_model::parse(&source).unwrap();
    let binary = root.join(".artifacts/idealis-check-go").join(driver);
    assert!(
        binary.exists(),
        "{} is missing: run just idealis-check-go",
        binary.display()
    );
    let mut process = Process::spawn(binary.to_str().unwrap(), &[fault]).unwrap();
    conform(&model, &mut process, Budget::default())
}

#[test]
#[ignore = "needs the Go drivers; run by just idealis-check-go"]
fn go_implementations_conform() {
    for (model, driver) in [
        ("fixtures/rmodel/permissions.model", "permissions"),
        ("fixtures/rmodel/sessions.model", "sessions"),
    ] {
        let report = run(model, driver, "");
        assert_eq!(report.status, Status::Matched, "{driver}: {report:?}");
        println!(
            "{driver}: {} configurations, {} transitions, {} calls",
            report.configurations, report.transitions, report.calls
        );
    }
}

#[test]
#[ignore = "needs the Go drivers; run by just idealis-check-go"]
fn go_faults_are_found() {
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
