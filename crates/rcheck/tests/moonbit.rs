//! Plan 0105: MoonBit implementations, written against the generated
//! traits and served on the JS target as JSON Lines drivers, conform to
//! the reference semantics, and the 0102 faults are found. `just
//! rcheck-moonbit` builds the drivers first.
use rcheck::conform::{Budget, Process, Status, conform};

fn run(model: &str, driver: &str, fault: &str) -> rcheck::conform::Report {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = std::fs::read_to_string(root.join(model)).unwrap();
    let model = rmodel::parse(&source).unwrap();
    let script = root.join(format!(
        "tests/rcheck/moonbit/_build/js/debug/build/cmd/{driver}_driver/{driver}_driver.js"
    ));
    assert!(
        script.exists(),
        "{} is missing: run just rcheck-moonbit",
        script.display()
    );
    let mut process = Process::spawn("node", &[script.to_str().unwrap(), fault]).unwrap();
    conform(&model, &mut process, Budget::default())
}

#[test]
#[ignore = "needs the MoonBit drivers; run by just rcheck-moonbit"]
fn moonbit_implementations_conform() {
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
#[ignore = "needs the MoonBit drivers; run by just rcheck-moonbit"]
fn moonbit_faults_are_found() {
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
