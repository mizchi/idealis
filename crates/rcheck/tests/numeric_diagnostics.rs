use std::path::Path;
use std::process::Command;

fn check(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rcheck"))
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn optional_numeric_operand_points_to_the_comparison_and_explains_presence() {
    let original = include_str!("../../../fixtures/rcheck/autosave.model");
    let path = std::env::temp_dir().join(format!(
        "rcheck-optional-number-{}.model",
        std::process::id()
    ));
    let replacement = "tab.pending >= revision, some tab.pending";
    std::fs::write(
        &path,
        original.replace("some tab.pending, tab.pending = revision", replacement),
    )
    .unwrap();
    let output = check(&path);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains(":15:11: Type: an integer comparison needs exactly one integer"),
        "{error}"
    );
    assert!(
        error.contains("Check presence with `some` before using it"),
        "{error}"
    );

    std::fs::write(
        &path,
        original.replace("tab.pending = revision", "tab.pending >= revision"),
    )
    .unwrap();
    let fixed = check(&path);
    assert!(
        fixed.status.success(),
        "{}",
        String::from_utf8_lossy(&fixed.stderr)
    );

    std::fs::write(
        &path,
        original.replace(
            "some tab.pending, tab.pending = revision",
            "some Tab.pending, Tab.pending >= revision",
        ),
    )
    .unwrap();
    let output = check(&path);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(":15:29: Type:"), "{error}");
    assert!(
        error.contains("may be empty or contain several integers"),
        "{error}"
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn duplicate_comparisons_in_a_component_point_to_the_unproven_operand() {
    let directory =
        std::env::temp_dir().join(format!("rcheck-number-component-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("main.model"), "model Linked\nvar sharedRevision: lone Nat\ninit { no sharedRevision }\nscope Int = 0..2, steps = 3").unwrap();
    std::fs::write(
        directory.join("component.model"),
        r#"// 取得済みの版
behavior @read() {
  guard {
    (some @rev and 0 <= @rev) or
    (0 <= @rev)
  }
}
"#,
    )
    .unwrap();
    let manifest = directory.join("project.rmodel.json");
    std::fs::write(&manifest, r#"{"fragments":["main.model",{"path":"component.model","namespace":"reader","ports":{"rev":{"state":"sharedRevision","access":"read"}}}]}"#).unwrap();
    let output = check(&manifest);
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("component.model:5:11: Type:"), "{error}");
    assert!(
        error.contains("Check presence with `some` before using it"),
        "{error}"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
