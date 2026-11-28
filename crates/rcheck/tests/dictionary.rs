use serde_json::{Value, json};
use std::{fs, path::Path, process::Command};

#[test]
fn a_project_resolves_dictionary_terms_and_rejects_stale_bindings() {
    let directory = std::env::temp_dir().join(format!("rcheck-dictionary-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let model = directory.join("loan.ideal");
    let manifest = directory.join("loan.rmodel.json");
    let dictionary = directory.join("dictionary.json");
    fs::write(
        &manifest,
        r#"{"dictionary":"dictionary.json","fragments":["loan.ideal"]}"#,
    )
    .unwrap();
    fs::write(
        &dictionary,
        json!({
            "id": "lending",
            "terms": [{
                "id": "active-member",
                "kind": "state",
                "name": {"ja": "有効な会員", "en": "Active member"},
                "definition": {"ja": "貸出資格がある会員。", "en": "A member eligible to borrow."}
            }]
        })
        .to_string(),
    )
    .unwrap();
    let source = r#"model Loan
enum Member = Ren
var active: set Member
init { active = Member }
term "active-member" for active
behavior close() { update { active = none } }
reachable Closed { no active }
scope steps = 2
"#;
    let command = |path: &Path, target: &str| {
        Command::new(env!("CARGO_BIN_EXE_rcheck"))
            .arg("generate")
            .arg(path)
            .arg(target)
            .output()
            .unwrap()
    };
    fs::write(&model, source).unwrap();
    let output = command(&manifest, "dictionary");
    assert!(output.status.success(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["id"], "lending");
    assert_eq!(
        report["terms"][0]["definition"]["ja"],
        "貸出資格がある会員。"
    );
    assert_eq!(report["terms"][0]["targets"], json!(["active"]));

    let native = directory.join("native.rmodel.json");
    fs::write(
        &native,
        r#"{"dictionary":"dictionary.ideal","fragments":["loan.ideal"]}"#,
    )
    .unwrap();
    fs::write(
        directory.join("dictionary.ideal"),
        r#"vocabulary "lending" {
  name = "Lending"
  term "active-member" {
    kind = state
    name = { ja: "有効な会員", en: "Active member" }
    description = { ja: "貸出資格がある会員。", en: "A member eligible to borrow." }
  }
}"#,
    )
    .unwrap();
    let resolved = command(&native, "dictionary");
    assert!(resolved.status.success(), "{resolved:?}");
    assert_eq!(
        serde_json::from_slice::<Value>(&resolved.stdout).unwrap(),
        report
    );

    let output = command(&native, "ir");
    assert!(output.status.success(), "{output:?}");
    let ir: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(ir["term_bindings"], json!({"active":"active-member"}));
    let mut invalid = ir;
    invalid["term_bindings"] = json!({"missing":"active-member"});
    assert!(
        rcheck::ir::parse(&invalid.to_string())
            .unwrap_err()
            .contains("unknown term target missing")
    );

    for (changed, expected) in [
        (
            source.replace("active-member", "missing"),
            "unknown dictionary term missing",
        ),
        (
            source.replace("for active", "for stale"),
            "unknown term target stale",
        ),
        (
            source.replace("for active", "for close"),
            "expects state, but close is action",
        ),
    ] {
        fs::write(&model, changed).unwrap();
        let rejected = command(&native, "dictionary");
        assert_eq!(rejected.status.code(), Some(2), "{rejected:?}");
        let stderr = String::from_utf8_lossy(&rejected.stderr);
        assert!(stderr.contains("loan.ideal:5:"), "{stderr}");
        assert!(stderr.contains(expected), "{stderr}");
    }
    fs::remove_dir_all(directory).unwrap();
}
