use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const AUTOSAVE: &str = include_str!("../../../fixtures/rcheck/autosave.model");

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/rcheck")
        .join(name)
}

fn invoke(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_idealis-check"))
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

fn first_report(output: &Output, status: i32) -> Value {
    assert_eq!(
        output.status.code(),
        Some(status),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Vec<Value>>(&output.stdout)
        .unwrap()
        .remove(0)
}

#[test]
fn autosave_ci_distinguishes_a_mismatch_from_an_incomplete_check() {
    let directory =
        std::env::temp_dir().join(format!("rcheck-expect-autosave-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("autosave.model");
    let check = ["--checker", "reference", "--question", "FinishDoesNotEdit"];
    let expect = [check.as_slice(), &["--expect", "holds"]].concat();
    for (source, verdict, status) in [
        (AUTOSAVE.to_owned(), "holds", 0),
        (
            AUTOSAVE.replace("unchanged(revision)", "before(revision) != after(revision)"),
            "violated",
            1,
        ),
        (
            AUTOSAVE.replace(
                "revision = 0, no pending",
                "revision = 0, revision = 1, no pending",
            ),
            "empty",
            1,
        ),
        (
            AUTOSAVE.replace("revision < 2", "revision <= 2"),
            "unknown",
            3,
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        let report = first_report(&invoke(&path, &expect), status);
        assert_eq!(report["answers"][0]["verdict"], verdict);
        assert_eq!(report, first_report(&invoke(&path, &check), 0));
    }
    std::fs::write(
        &path,
        format!("{AUTOSAVE}\nreachable BothVersions {{ revision = 0 and revision = 1 }}"),
    )
    .unwrap();
    let no_path = invoke(
        &path,
        &["--question", "BothVersions", "--expect", "unreachable"],
    );
    assert_eq!(
        first_report(&no_path, 0)["answers"][0]["verdict"],
        "unreachable"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn bounded_absence_cannot_pass_but_a_bounded_witness_can() {
    let mutex = fixture("../rmodel/mutex.model");
    let valid = invoke(
        &mutex,
        &[
            "--checker",
            "sat",
            "--question",
            "OneHolder",
            "--expect",
            "holds",
        ],
    );
    let report = first_report(&valid, 3);
    assert_eq!(report["answers"][0]["verdict"], "holds");
    assert_eq!(report["answers"][0]["coverage"]["kind"], "bounded");

    let violation = invoke(
        &mutex,
        &[
            "--checker",
            "sat",
            "--question",
            "NeverHeld",
            "--expect",
            "violated",
        ],
    );
    assert_eq!(
        first_report(&violation, 0)["answers"][0]["verdict"],
        "violated"
    );

    let witness = invoke(
        &mutex,
        &[
            "--checker",
            "sat",
            "--question",
            "Handover",
            "--expect",
            "reachable",
        ],
    );
    let report = first_report(&witness, 0);
    assert_eq!(report["answers"][0]["verdict"], "reachable");
    assert_eq!(report["answers"][0]["coverage"]["kind"], "bounded");

    let all = invoke(
        &mutex,
        &[
            "--checker",
            "all",
            "--question",
            "OneHolder",
            "--expect",
            "holds",
        ],
    );
    let report = first_report(&all, 0);
    assert_eq!(report["answers"].as_array().unwrap().len(), 4);
}

#[test]
fn expected_verdict_requires_one_question_and_a_definite_answer() {
    let path = fixture("autosave.model");
    for args in [
        vec!["--expect", "holds"],
        vec!["--question", "FinishDoesNotEdit", "--expect", "unknown"],
        vec!["--question", "FinishDoesNotEdit", "--expect", "empty"],
        vec![
            "--question",
            "FinishDoesNotEdit",
            "--expect",
            "holds",
            "--expect",
            "reachable",
        ],
        vec![
            "--question",
            "FinishDoesNotEdit",
            "--checker",
            "sat",
            "--expect",
            "holds",
        ],
    ] {
        let output = invoke(&path, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
}
