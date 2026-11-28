use serde_json::Value;
use std::path::Path;
use std::process::{Command, Output};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn invoke(model: &Path, oracle: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .arg("lean-oracle")
        .arg(model)
        .arg(oracle)
        .args(args)
        .output()
        .unwrap()
}

fn decode(output: Output) -> (i32, Value) {
    let json = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&output.stderr)));
    (output.status.code().unwrap(), json)
}

fn check(model: &Path, args: &[&str]) -> (i32, Value) {
    decode(invoke(model, &root().join("formal/rmodel/Bank.lean"), args))
}

#[test]
#[ignore = "needs Lean on PATH; run by just idealis-check-lean-oracle"]
fn lean_checks_reachable_states_and_reports_a_counterexample() {
    let bank = root().join("fixtures/rcheck/bank.model");
    let (code, normal) = check(&bank, &[]);
    assert_eq!(code, 0, "{normal}");
    assert_eq!(normal["status"], "holds");
    assert_eq!(normal["observations"], 25);

    let fault = std::fs::read_to_string(&bank)
        .unwrap()
        .replace("a.balance + n <= 4", "a.balance + n <= 5")
        .replace("Int = -4..4", "Int = -4..5");
    let file = std::env::temp_dir().join(format!(
        "idealis-check-lean-oracle-{}.model",
        std::process::id()
    ));
    std::fs::write(&file, fault).unwrap();
    let (code, report) = check(&file, &[]);
    std::fs::remove_file(file).unwrap();
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["status"], "violated");
    assert!(
        report["counterexample"]["state"]["balance"]
            .as_object()
            .unwrap()
            .values()
            .any(|value| value == 5),
        "{report}"
    );
}

#[test]
fn incomplete_search_returns_unknown_before_starting_lean() {
    let bank = root().join("fixtures/rcheck/bank.model");
    let (code, report) = decode(invoke(
        &bank,
        &root().join("formal/rmodel/missing.lean"),
        &["--max-states", "1"],
    ));
    assert_eq!(code, 3, "{report}");
    assert_eq!(report["status"], "unknown");
}

#[test]
fn empty_model_returns_empty_before_starting_lean() {
    let path = std::env::temp_dir().join(format!("rcheck-lean-empty-{}.model", std::process::id()));
    std::fs::write(
        &path,
        "model Empty\nenum Phase = On | Off\nvar phase: one Phase\ninit { phase = On phase = Off }\nscope steps = 1",
    )
    .unwrap();
    let output = invoke(&path, &root().join("formal/rmodel/missing.lean"), &[]);
    std::fs::remove_file(path).unwrap();
    let (code, report) = decode(output);
    assert_eq!(code, 3, "{report}");
    assert_eq!(report["status"], "empty");
    assert_eq!(report["observations"], 0);
}

#[test]
#[ignore = "needs Lean on PATH; run by just idealis-check-lean-oracle"]
fn incomplete_answers_and_lean_errors_are_not_counterexamples() {
    let model = root().join("fixtures/rcheck/bank.model");
    let oracle = std::env::temp_dir().join(format!("RcheckOracle{}.lean", std::process::id()));
    for (source, diagnostic) in [
        (
            "def main : IO Unit := IO.println \"[]\"\n",
            "Lean returned 0 answers for 25 observations",
        ),
        ("theorem impossible : False := by decide\n", "Lean failed"),
    ] {
        std::fs::write(&oracle, source).unwrap();
        let output = invoke(&model, &oracle, &[]);
        std::fs::remove_file(&oracle).unwrap();
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(diagnostic), "{stderr}");
    }
}
