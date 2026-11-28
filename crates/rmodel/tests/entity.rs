const SOURCE: &str = r#"
model Ledger
entity Account "残高を持つ口座。引用: \"口座\"。"
var open: set Account
init { no open }
action add(a: Account) { open' = open + a }
reachable Opened { some open }
scope Account = 2, steps = 3
"#;

#[test]
fn entity_definition_generates_both_types_and_has_signature_semantics() {
    let model = rmodel::parse(SOURCE).unwrap();
    let old = rmodel::parse(&SOURCE.replace(
        r#"entity Account "残高を持つ口座。引用: \"口座\"。""#,
        "sig Account",
    ))
    .unwrap();
    let definition = "残高を持つ口座。引用: \"口座\"。";
    assert!(rmodel::bindings::rust(&model, SOURCE).contains(&format!("/// {definition}\n")));
    assert!(rmodel::bindings::typescript(&model).contains(&format!("/** {definition} */\n")));
    assert_eq!(
        rmodel::reference::evaluate(&model, 100),
        rmodel::reference::evaluate(&old, 100)
    );
    assert_eq!(rmodel::to_alloy(&model), rmodel::to_alloy(&old));
    assert_eq!(
        rmodel::to_tla(&model, "Opened").unwrap(),
        rmodel::to_tla(&old, "Opened").unwrap()
    );
}

#[test]
fn entity_names_are_checked_at_declarations_and_references() {
    for change in [
        SOURCE.replace("var open:", "sig Account\nvar open:"),
        SOURCE.replace("set Account", "set Acccount"),
        SOURCE.replace(
            r#"entity Account "残高を持つ口座。引用: \"口座\"。""#,
            "entity Account \"\"",
        ),
    ] {
        assert!(rmodel::parse(&change).is_err(), "{change}");
    }
}

#[test]
fn definition_text_remains_text_in_generated_sources() {
    let source = SOURCE.replace("残高を持つ口座", "説明 */ 終わり") + "\n// \"####\n";
    let model = rmodel::parse(&source).unwrap();
    let rust = rmodel::bindings::rust(&model, &source);
    assert!(rust.contains(&format!("pub const MODEL: &str = {:?};", source.trim_end())));
    let typescript = rmodel::bindings::typescript(&model);
    assert!(typescript.contains("/** 説明 *&#47; 終わり"));
}

#[test]
fn descriptions_preserve_languages_lines_and_literal_comment_markers() {
    let source = r#"model Ledger
// Type names are shared across languages.
entity Account description = {
  default: "Account",
  ja: """残高を持つ口座。
// この行は説明の一部。 */""",
  "EN-US": "An account that holds a balance."
}
scope Account = 1, steps = 2 // one account is enough
"#;
    let model = rmodel::parse(source).unwrap();
    assert_eq!(model.entity_definitions["Account"], "Account");
    let rust = rmodel::bindings::rust(&model, source);
    assert!(rust.contains("/// [ja] 残高を持つ口座。\n/// // この行は説明の一部。 */\n"));
    assert!(rust.contains("/// [en-us] An account that holds a balance.\n"));
    let ts = rmodel::bindings::typescript(&model);
    assert!(ts.contains("[ja] 残高を持つ口座。\n * // この行は説明の一部。 *&#47;"));
    let plain = rmodel::parse("model Ledger entity Account scope Account = 1, steps = 2").unwrap();
    assert_eq!(rmodel::to_alloy(&model), rmodel::to_alloy(&plain));
}

#[test]
fn triple_quoted_default_preserves_line_breaks_on_windows() {
    let source = "model Ledger\r\nentity Account description = \"\"\"First\r\nSecond\"\"\"\r\nscope Account = 1, steps = 2";
    let model = rmodel::parse(source).unwrap();
    assert_eq!(model.entity_definitions["Account"], "First\nSecond");
}

#[test]
fn invalid_descriptions_are_rejected() {
    for description in [
        r#"{ en: "one", EN: "two" }"#,
        r#"{ "en__US": "bad tag" }"#,
        r#"{ ja: "  " }"#,
        r#"{}"#,
        r#""""unterminated"#,
    ] {
        let source = format!(
            "model Ledger entity Account description = {description}\nscope Account = 1, steps = 2"
        );
        assert!(rmodel::parse(&source).is_err(), "{description}");
    }
}
