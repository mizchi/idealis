const SOURCE: &str = r#"vocabulary "example-tools" {
  name = { ja: "処理系", en: "Tools" }
  // A name may be shared in both languages.
  term "source" { kind = language name = "Source" description = "Source text." }
  term "program" { kind = artifact name = "Program" description = "A checked program." }
  term "compiler" {
    kind = compiler
    name = "Compiler"
    input = ["source"]
    output = ["program"]
    description = { ja: """入力を読む。
プログラムを生成する。""", en: "Compiles source." }
    code = { path: "src/main.rs", symbol: "compile" }
  }
  term "checker" {
    kind = checker
    name = "Checker"
    input = ["program"]
    compared_with = ["reference"]
    description = "Checks programs."
  }
  term "reference" { kind = checker name = "Reference" description = "Reference checker." }
}"#;

#[test]
fn a_checked_vocabulary_can_be_read_as_spec_terms() {
    let module = idealis_spec::compile_vocabulary(SOURCE).unwrap();
    let spec = idealis_spec::from_json(&module.to_string()).unwrap();
    assert_eq!(spec.id, "example-tools");
    let compiler = spec
        .terms
        .iter()
        .find(|term| term.id == "compiler")
        .unwrap();
    assert_eq!(
        compiler.definition.ja,
        "入力を読む。\nプログラムを生成する。"
    );
    assert_eq!(compiler.code[0].symbol, "compile");
    let flow = compiler.flow.as_ref().unwrap();
    assert_eq!(flow.role, "compiler");
    assert_eq!(flow.output, ["program"]);
    let uses = idealis_spec::vocabulary_uses(&spec, "program").unwrap();
    assert_eq!(uses["producers"], serde_json::json!(["compiler"]));
    assert_eq!(uses["consumers"], serde_json::json!(["checker"]));
}

#[test]
fn rejects_broken_links_and_incompatible_roles() {
    for (old, new, expected) in [
        (
            r#"input = ["program"]"#,
            r#"input = ["missing"]"#,
            "unknown term missing",
        ),
        (
            r#"output = ["program"]"#,
            r#"output = ["source"]"#,
            "output requires an artifact",
        ),
        (
            r#"compared_with = ["reference"]"#,
            r#"compared_with = ["compiler"]"#,
            "compared_with requires a checker",
        ),
        (
            r#"input = ["source"]"#,
            r#"input = ["source", "source"]"#,
            "duplicate input reference source",
        ),
    ] {
        let changed = SOURCE.replace(old, new);
        let error = idealis_spec::compile_vocabulary(&changed).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[test]
fn domain_terms_keep_their_model_kind_in_spec_json() {
    let source = r#"vocabulary "lending" {
  name = "Lending"
  term "eligibility" { kind = state name = "Eligibility" description = "Can borrow." }
  term "suspend" { kind = action name = "Suspend" description = "Revokes eligibility." }
}"#;
    let compiled = idealis_spec::compile_vocabulary(source).unwrap();
    let spec = idealis_spec::from_json(&compiled.to_string()).unwrap();
    assert_eq!(spec.terms[0].kind, "state");
    assert_eq!(spec.terms[1].kind, "action");
    assert!(spec.terms.iter().all(|term| term.flow.is_none()));
}
